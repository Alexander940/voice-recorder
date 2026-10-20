//! Orígenes de audio: micrófono (cpal) y archivo WAV (pruebas).

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::AudioError;

/// Recibe bloques de muestras entrelazadas en `f32`.
pub type SampleCallback = Box<dyn FnMut(&[f32]) + Send + 'static>;

pub trait AudioSource {
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn start(&mut self, on_samples: SampleCallback) -> Result<(), AudioError>;
    fn stop(&mut self);
}

/// Reproduce un WAV como si fuera el micrófono, en bloques de 1024 frames.
pub struct FileAudioSource {
    path: PathBuf,
    spec: hound::WavSpec,
}

impl FileAudioSource {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AudioError> {
        let path = path.into();
        let spec = hound::WavReader::open(&path)
            .map_err(|e| AudioError(e.to_string()))?
            .spec();
        Ok(Self { path, spec })
    }
}

impl AudioSource for FileAudioSource {
    fn sample_rate(&self) -> u32 {
        self.spec.sample_rate
    }

    fn channels(&self) -> u16 {
        self.spec.channels
    }

    fn start(&mut self, mut on_samples: SampleCallback) -> Result<(), AudioError> {
        let mut reader =
            hound::WavReader::open(&self.path).map_err(|e| AudioError(e.to_string()))?;
        let samples: Vec<f32> = match self.spec.sample_format {
            hound::SampleFormat::Float => {
                reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
            }
            hound::SampleFormat::Int => {
                let scale = (1_i64 << (self.spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.unwrap_or(0) as f32 / scale)
                    .collect()
            }
        };
        for chunk in samples.chunks(1024 * self.spec.channels as usize) {
            on_samples(chunk);
        }
        Ok(())
    }

    fn stop(&mut self) {}
}

/// Micrófono real. El stream de cpal vive en su propio hilo porque no es `Send`
/// en todas las plataformas.
pub struct CpalSource {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    worker: Option<(mpsc::Sender<()>, JoinHandle<()>)>,
}

/// Nombres de los micrófonos disponibles.
pub fn input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devs| {
            devs.filter_map(|d| d.description().ok().map(|desc| desc.name().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

impl CpalSource {
    /// Usa el micrófono con ese nombre, o el predeterminado si es `None` o no existe.
    pub fn new(device_name: Option<&str>) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let named = device_name.and_then(|name| {
            host.input_devices().ok()?.find(|d| {
                d.description()
                    .map(|desc| desc.name() == name)
                    .unwrap_or(false)
            })
        });
        let device = named
            .or_else(|| host.default_input_device())
            .ok_or_else(|| AudioError("No se encontró ningún micrófono".into()))?;
        let config = device
            .default_input_config()
            .map_err(|e| AudioError(e.to_string()))?;
        Ok(Self {
            device,
            config,
            worker: None,
        })
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    mut cb: SampleCallback,
) -> Result<cpal::Stream, AudioError> {
    let stream_config: cpal::StreamConfig = (*config).into();
    let on_err = |e| eprintln!("error de audio: {e}");
    let to_err = |e: cpal::Error| AudioError(e.to_string());
    match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_input_stream(stream_config, move |d: &[f32], _: &_| cb(d), on_err, None)
            .map_err(to_err),
        cpal::SampleFormat::I16 => device
            .build_input_stream(
                stream_config,
                move |d: &[i16], _: &_| {
                    let v: Vec<f32> = d.iter().map(|s| *s as f32 / 32_768.0).collect();
                    cb(&v)
                },
                on_err,
                None,
            )
            .map_err(to_err),
        cpal::SampleFormat::U16 => device
            .build_input_stream(
                stream_config,
                move |d: &[u16], _: &_| {
                    let v: Vec<f32> = d
                        .iter()
                        .map(|s| (*s as f32 - 32_768.0) / 32_768.0)
                        .collect();
                    cb(&v)
                },
                on_err,
                None,
            )
            .map_err(to_err),
        cpal::SampleFormat::I32 => device
            .build_input_stream(
                stream_config,
                move |d: &[i32], _: &_| {
                    let v: Vec<f32> = d.iter().map(|s| *s as f32 / 2_147_483_648.0).collect();
                    cb(&v)
                },
                on_err,
                None,
            )
            .map_err(to_err),
        other => Err(AudioError(format!(
            "formato de micrófono no soportado: {other:?}"
        ))),
    }
}

impl AudioSource for CpalSource {
    fn sample_rate(&self) -> u32 {
        self.config.sample_rate()
    }

    fn channels(&self) -> u16 {
        self.config.channels()
    }

    fn start(&mut self, on_samples: SampleCallback) -> Result<(), AudioError> {
        self.stop();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), AudioError>>();
        let device = self.device.clone();
        let config = self.config;
        let handle = std::thread::spawn(move || {
            let stream = match build_stream(&device, &config, on_samples) {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(AudioError(e.to_string())));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            // Mantiene el stream vivo hasta que se pida parar.
            let _ = stop_rx.recv();
        });
        ready_rx
            .recv()
            .map_err(|_| AudioError("el hilo de audio terminó inesperadamente".into()))??;
        self.worker = Some((stop_tx, handle));
        Ok(())
    }

    fn stop(&mut self) {
        if let Some((tx, handle)) = self.worker.take() {
            let _ = tx.send(());
            let _ = handle.join();
        }
    }
}

impl Drop for CpalSource {
    fn drop(&mut self) {
        self.stop();
    }
}
