//! Sonidos opcionales de inicio, fin y error (HU-13). Tonos generados en memoria.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Start,
    Stop,
    Error,
}

impl Sound {
    /// (frecuencia Hz, duración ms) de cada nota.
    fn notes(self) -> &'static [(f32, u64)] {
        match self {
            Sound::Start => &[(660.0, 70), (880.0, 90)],
            Sound::Stop => &[(880.0, 70), (660.0, 90)],
            Sound::Error => &[(330.0, 120), (247.0, 180)],
        }
    }
}

/// Muestras mono f32 con fundido de 5 ms para evitar chasquidos.
pub fn render(sound: Sound, sample_rate: u32, volume: f32) -> Vec<f32> {
    let fade = (sample_rate as f32 * 0.005) as usize;
    sound
        .notes()
        .iter()
        .flat_map(|&(freq, ms)| {
            let n = (sample_rate as u64 * ms / 1000) as usize;
            (0..n).map(move |i| {
                let env = (i.min(n - 1 - i) as f32 / fade as f32).min(1.0);
                let t = i as f32 / sample_rate as f32;
                volume * env * (2.0 * std::f32::consts::PI * freq * t).sin()
            })
        })
        .collect()
}

pub fn duration(sound: Sound) -> Duration {
    Duration::from_millis(sound.notes().iter().map(|(_, ms)| ms).sum())
}

/// Reproduce en la salida predeterminada, en su propio hilo. Los fallos se ignoran:
/// un sonido nunca debe romper un dictado.
pub fn play(sound: Sound) {
    std::thread::spawn(move || {
        if let Err(e) = play_blocking(sound) {
            tracing::debug!(error = %e, "no se pudo reproducir el sonido");
        }
    });
}

fn play_blocking(sound: Sound) -> Result<(), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("sin salida de audio")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err("formato de salida no soportado".into());
    }
    let channels = config.channels() as usize;
    let samples = Arc::new(Mutex::new(
        render(sound, config.sample_rate(), 0.25).into_iter(),
    ));
    let source = samples.clone();
    let stream = device
        .build_output_stream(
            config.into(),
            move |out: &mut [f32], _: &_| {
                let mut it = source.lock().expect("sound poisoned");
                for frame in out.chunks_mut(channels) {
                    let v = it.next().unwrap_or(0.0);
                    frame.fill(v);
                }
            },
            |e| tracing::debug!(error = %e, "error de salida de audio"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    std::thread::sleep(duration(sound) + Duration::from_millis(80));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_length_matches_notes() {
        for s in [Sound::Start, Sound::Stop, Sound::Error] {
            let samples = render(s, 48_000, 0.25);
            let expected = duration(s).as_secs_f64() * 48_000.0;
            assert!((samples.len() as f64 - expected).abs() <= 2.0);
        }
    }

    #[test]
    fn volume_is_bounded_and_edges_fade_in_and_out() {
        let samples = render(Sound::Error, 16_000, 0.25);
        assert!(samples.iter().all(|s| s.abs() <= 0.25 + 1e-6));
        assert_eq!(samples[0], 0.0);
        assert!(samples.last().unwrap().abs() < 0.01);
    }

    #[test]
    fn sounds_are_distinguishable() {
        assert_ne!(Sound::Start.notes(), Sound::Stop.notes());
        assert!(duration(Sound::Error) > duration(Sound::Start));
    }

    /// Reproduce los tres sonidos por los altavoces. A mano:
    /// `cargo test -- --ignored real_speakers`
    #[test]
    #[ignore = "usa la salida de audio real"]
    fn real_speakers_play_all_sounds() {
        for s in [Sound::Start, Sound::Stop, Sound::Error] {
            play_blocking(s).unwrap();
        }
    }
}
