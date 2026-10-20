//! Procesamiento puro: mezcla a mono, remuestreo, nivel y WAV.

use std::io::Cursor;
use std::time::Duration;

use super::AudioError;

/// Formato que espera Whisper.
pub const TARGET_RATE: u32 = 16_000;
/// Grabaciones más cortas se descartan.
pub const MIN_DURATION: Duration = Duration::from_millis(300);
/// Límite por defecto de una grabación.
pub const DEFAULT_MAX_DURATION: Duration = Duration::from_secs(5 * 60);

/// Promedia los canales de audio entrelazado.
pub fn to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    interleaved
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Remuestrea audio mono. Al bajar de frecuencia promedia cada ventana (filtro
/// paso bajo sencillo, suficiente para voz); al subir interpola linealmente.
pub fn resample(mono: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || mono.is_empty() {
        return mono.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (mono.len() as f64 / ratio).round() as usize;
    (0..out_len)
        .map(|i| {
            let start = i as f64 * ratio;
            if ratio > 1.0 {
                let a = start.floor() as usize;
                let b = (((i + 1) as f64 * ratio).floor() as usize).clamp(a + 1, mono.len());
                let window = &mono[a.min(mono.len() - 1)..b];
                window.iter().sum::<f32>() / window.len() as f32
            } else {
                let a = start.floor() as usize;
                let frac = (start - a as f64) as f32;
                let s0 = mono[a.min(mono.len() - 1)];
                let s1 = mono[(a + 1).min(mono.len() - 1)];
                s0 + (s1 - s0) * frac
            }
        })
        .collect()
}

/// Raíz cuadrática media de un bloque de muestras.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Nivel 0..1 para la UI: escala logarítmica de -60 dBFS a 0 dBFS.
pub fn display_level(rms: f32) -> f32 {
    if rms <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Codifica audio mono a 16 kHz como WAV PCM de 16 bits, en memoria.
pub fn encode_wav(samples: &[f32]) -> Result<Vec<u8>, AudioError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).map_err(|e| AudioError(e.to_string()))?;
        for s in samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(v)
                .map_err(|e| AudioError(e.to_string()))?;
        }
        writer.finalize().map_err(|e| AudioError(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

/// Resultado de añadir un bloque a la grabación.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PushOutcome {
    pub rms: f32,
    /// Se alcanzó el límite: el resto del bloque se descartó.
    pub limit_reached: bool,
}

/// Acumula audio entrelazado en su formato nativo hasta terminar.
#[derive(Debug)]
pub struct Recording {
    sample_rate: u32,
    channels: u16,
    max_frames: usize,
    samples: Vec<f32>,
}

impl Recording {
    pub fn new(sample_rate: u32, channels: u16, max: Duration) -> Self {
        Self {
            sample_rate,
            channels: channels.max(1),
            max_frames: (max.as_secs_f64() * sample_rate as f64) as usize,
            samples: Vec::new(),
        }
    }

    fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames() as f64 / self.sample_rate as f64)
    }

    pub fn push(&mut self, interleaved: &[f32]) -> PushOutcome {
        let room = self.max_frames.saturating_sub(self.frames()) * self.channels as usize;
        let take = interleaved.len().min(room);
        self.samples.extend_from_slice(&interleaved[..take]);
        PushOutcome {
            rms: rms(interleaved),
            limit_reached: self.frames() >= self.max_frames,
        }
    }

    /// WAV de 16 kHz mono, o `None` si la grabación es demasiado corta.
    pub fn finish(self) -> Result<Option<Vec<u8>>, AudioError> {
        if self.duration() < MIN_DURATION {
            return Ok(None);
        }
        let mono = to_mono(&self.samples, self.channels);
        let resampled = resample(&mono, self.sample_rate, TARGET_RATE);
        encode_wav(&resampled).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, channels: u16, secs: f64, freq: f32, amp: f32) -> Vec<f32> {
        let frames = (rate as f64 * secs) as usize;
        (0..frames)
            .flat_map(|i| {
                let v = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin();
                std::iter::repeat_n(v, channels as usize)
            })
            .collect()
    }

    #[test]
    fn to_mono_averages_channels() {
        assert_eq!(to_mono(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
        assert_eq!(to_mono(&[0.1, 0.2], 1), vec![0.1, 0.2]);
    }

    #[test]
    fn resample_changes_length_by_ratio() {
        let one_sec_48k = vec![0.0; 48_000];
        assert_eq!(resample(&one_sec_48k, 48_000, 16_000).len(), 16_000);
        let one_sec_44k = vec![0.0; 44_100];
        assert_eq!(resample(&one_sec_44k, 44_100, 16_000).len(), 16_000);
        let one_sec_8k = vec![0.0; 8_000];
        assert_eq!(resample(&one_sec_8k, 8_000, 16_000).len(), 16_000);
    }

    #[test]
    fn resample_preserves_constant_signal() {
        let out = resample(&[0.25; 4410], 44_100, 16_000);
        assert!(out.iter().all(|s| (s - 0.25).abs() < 1e-6));
    }

    #[test]
    fn rms_of_known_signals() {
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[0.5; 100]) - 0.5).abs() < 1e-6);
        let s = sine(16_000, 1, 1.0, 440.0, 1.0);
        assert!((rms(&s) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01);
    }

    #[test]
    fn display_level_maps_db_range() {
        assert_eq!(display_level(0.0), 0.0);
        assert_eq!(display_level(1.0), 1.0);
        assert!((display_level(0.001) - 0.0).abs() < 1e-6); // -60 dB
        assert!((display_level(0.031_622_8) - 0.5).abs() < 0.01); // -30 dB
    }

    #[test]
    fn recording_discards_short_audio() {
        let mut r = Recording::new(48_000, 1, DEFAULT_MAX_DURATION);
        r.push(&sine(48_000, 1, 0.2, 440.0, 0.5));
        assert_eq!(r.finish().unwrap(), None);
    }

    #[test]
    fn recording_stops_at_limit() {
        let mut r = Recording::new(16_000, 2, Duration::from_secs(1));
        let first = r.push(&sine(16_000, 2, 0.6, 440.0, 0.5));
        assert!(!first.limit_reached);
        let second = r.push(&sine(16_000, 2, 0.6, 440.0, 0.5));
        assert!(second.limit_reached);
        assert_eq!(r.duration(), Duration::from_secs(1));
    }

    #[test]
    fn push_reports_block_rms() {
        let mut r = Recording::new(16_000, 1, DEFAULT_MAX_DURATION);
        assert!((r.push(&[0.5; 160]).rms - 0.5).abs() < 1e-6);
    }
}
