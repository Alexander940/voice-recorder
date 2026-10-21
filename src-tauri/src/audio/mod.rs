//! Captura de micrófono y codificación a WAV 16 kHz mono (HU-04).

mod processing;
mod source;

pub use processing::{
    DEFAULT_MAX_DURATION, MIN_DURATION, PushOutcome, Recording, Segment, TARGET_RATE,
    display_level, encode_wav, resample, rms, to_mono,
};
pub use source::{AudioSource, CpalSource, FileAudioSource, SampleCallback, input_device_names};

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioError(pub String);

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AudioError {}

/// Evento de nivel para el overlay, como mucho cada `LEVEL_INTERVAL`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct LevelEvent {
    pub level: f32,
    pub elapsed_ms: u64,
}

pub const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

/// Resultado de parar una grabación.
#[derive(Debug)]
pub struct Stopped {
    /// Hubo trozos entregados antes de parar (grabación larga).
    pub chunked: bool,
    pub total: Duration,
    /// Audio pendiente: toda la grabación, o lo grabado desde el último trozo.
    pub rest: Option<Segment>,
}

/// Saca trozos de una grabación en curso sin pararla.
#[derive(Clone)]
pub struct Chunker(Arc<Mutex<Option<Recording>>>);

impl Chunker {
    /// El siguiente trozo si ya hay `len` de audio nuevo. Solo copia muestras
    /// bajo el candado; codificar es cosa de quien llama.
    pub fn take(&self, len: Duration, overlap: Duration) -> Option<Segment> {
        self.0
            .lock()
            .ok()?
            .as_mut()
            .and_then(|r| r.take_chunk(len, overlap))
    }
}

/// Grabación en curso sobre cualquier `AudioSource`.
pub struct Recorder<S: AudioSource> {
    source: S,
    recording: Arc<Mutex<Option<Recording>>>,
}

impl<S: AudioSource> Recorder<S> {
    /// Empieza a grabar. `on_level` recibe el nivel periódicamente y `on_limit`
    /// se llama una vez si se alcanza la duración máxima.
    pub fn start(
        mut source: S,
        max: Duration,
        mut on_level: impl FnMut(LevelEvent) + Send + 'static,
        on_limit: impl FnOnce() + Send + 'static,
    ) -> Result<Self, AudioError> {
        let recording = Arc::new(Mutex::new(Some(Recording::new(
            source.sample_rate(),
            source.channels(),
            max,
        ))));
        let sink = recording.clone();
        let started = Instant::now();
        let mut last_level: Option<Instant> = None;
        let mut on_limit = Some(on_limit);
        source.start(Box::new(move |samples| {
            let outcome = match sink
                .lock()
                .ok()
                .and_then(|mut r| r.as_mut().map(|r| r.push(samples)))
            {
                Some(o) => o,
                None => return,
            };
            let now = Instant::now();
            if last_level.is_none_or(|t| now - t >= LEVEL_INTERVAL) {
                last_level = Some(now);
                on_level(LevelEvent {
                    level: display_level(outcome.rms),
                    elapsed_ms: (now - started).as_millis() as u64,
                });
            }
            if outcome.limit_reached
                && let Some(f) = on_limit.take()
            {
                f();
            }
        }))?;
        Ok(Self { source, recording })
    }

    /// Para y devuelve el WAV, o `None` si fue demasiado corto.
    pub fn stop(self) -> Result<Option<Vec<u8>>, AudioError> {
        self.stop_chunked()?.rest.map(|s| s.encode()).transpose()
    }

    /// Para y devuelve el audio que falta por entregar y si hubo trozos antes.
    pub fn stop_chunked(mut self) -> Result<Stopped, AudioError> {
        self.source.stop();
        let recording = self
            .recording
            .lock()
            .map_err(|_| AudioError("grabación corrupta".into()))?
            .take();
        Ok(match recording {
            Some(r) => Stopped {
                chunked: r.is_chunked(),
                total: r.duration(),
                rest: r.finish_segment(),
            },
            None => Stopped {
                chunked: false,
                total: Duration::ZERO,
                rest: None,
            },
        })
    }

    /// Acceso a la grabación en curso para ir sacando trozos desde otro hilo.
    pub fn chunker(&self) -> Chunker {
        Chunker(self.recording.clone())
    }

    /// Para y descarta el audio.
    pub fn cancel(mut self) {
        self.source.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn write_sine_wav(path: &Path, rate: u32, channels: u16, secs: f64) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        let frames = (rate as f64 * secs) as usize;
        for i in 0..frames {
            let v = (0.5
                * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin()
                * i16::MAX as f32) as i16;
            for _ in 0..channels {
                w.write_sample(v).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    fn record_fixture(rate: u32, channels: u16, secs: f64, max: Duration) -> Option<Vec<u8>> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.wav");
        write_sine_wav(&path, rate, channels, secs);
        let source = FileAudioSource::open(&path).unwrap();
        let recorder = Recorder::start(source, max, |_| {}, || {}).unwrap();
        recorder.stop().unwrap()
    }

    fn assert_16k_mono(wav: &[u8], expected_secs: f64) {
        let reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16_000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);
        let secs = reader.duration() as f64 / 16_000.0;
        assert!(
            (secs - expected_secs).abs() / expected_secs <= 0.01,
            "duración {secs} vs {expected_secs}"
        );
    }

    #[test]
    fn fixture_44100_stereo_becomes_16k_mono() {
        let wav = record_fixture(44_100, 2, 2.0, DEFAULT_MAX_DURATION).unwrap();
        assert_16k_mono(&wav, 2.0);
    }

    #[test]
    fn fixture_48000_mono_becomes_16k_mono() {
        let wav = record_fixture(48_000, 1, 1.5, DEFAULT_MAX_DURATION).unwrap();
        assert_16k_mono(&wav, 1.5);
    }

    #[test]
    fn fixture_200ms_is_discarded() {
        assert_eq!(record_fixture(48_000, 1, 0.2, DEFAULT_MAX_DURATION), None);
    }

    #[test]
    fn fixture_longer_than_limit_is_cut_and_notifies_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long.wav");
        write_sine_wav(&path, 16_000, 1, 3.0);
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let recorder = Recorder::start(
            FileAudioSource::open(&path).unwrap(),
            Duration::from_secs(1),
            |_| {},
            move || {
                h.fetch_add(1, Ordering::SeqCst);
            },
        )
        .unwrap();
        let wav = recorder.stop().unwrap().unwrap();
        assert_16k_mono(&wav, 1.0);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn chunker_takes_chunks_while_recording_and_stop_returns_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long.wav");
        write_sine_wav(&path, 16_000, 1, 3.0);
        let recorder = Recorder::start(
            FileAudioSource::open(&path).unwrap(),
            DEFAULT_MAX_DURATION,
            |_| {},
            || {},
        )
        .unwrap();
        let chunker = recorder.chunker();
        let first = chunker
            .take(Duration::from_secs(2), Duration::from_millis(500))
            .unwrap();
        assert_eq!(first.samples.len(), 32_000);
        let stopped = recorder.stop_chunked().unwrap();
        assert!(stopped.chunked);
        assert_eq!(stopped.total, Duration::from_secs(3));
        // 1 s nuevo + 0,5 s de solape.
        assert_eq!(stopped.rest.unwrap().samples.len(), 24_000);
        // Tras parar, el chunker ya no entrega nada.
        assert!(chunker.take(Duration::ZERO, Duration::ZERO).is_none());
    }

    #[test]
    fn level_events_are_emitted_and_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lvl.wav");
        write_sine_wav(&path, 16_000, 1, 0.5);
        let levels = Arc::new(Mutex::new(Vec::new()));
        let l = levels.clone();
        let recorder = Recorder::start(
            FileAudioSource::open(&path).unwrap(),
            DEFAULT_MAX_DURATION,
            move |e| l.lock().unwrap().push(e),
            || {},
        )
        .unwrap();
        recorder.cancel();
        let levels = levels.lock().unwrap();
        assert!(!levels.is_empty());
        assert!(levels.iter().all(|e| (0.0..=1.0).contains(&e.level)));
        // Seno de amplitud 0.5 ≈ -9 dBFS → nivel ≈ 0.85
        assert!(levels[0].level > 0.7);
    }
}

#[cfg(test)]
mod hardware_tests {
    use super::*;

    /// Graba 1 s del micrófono predeterminado. Se ejecuta a mano:
    /// `cargo test -- --ignored real_microphone`
    #[test]
    #[ignore = "usa el micrófono real"]
    fn real_microphone_records_one_second() {
        let source = CpalSource::new(None).expect("micrófono");
        let recorder = Recorder::start(source, DEFAULT_MAX_DURATION, |_| {}, || {}).unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        let wav = recorder.stop().unwrap().expect("1 s supera el mínimo");
        let reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, TARGET_RATE);
        assert_eq!(reader.spec().channels, 1);
        let secs = reader.duration() as f64 / TARGET_RATE as f64;
        assert!((0.8..=1.2).contains(&secs), "duración {secs}");
    }
}
