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

/// Trozo de audio mono a la frecuencia nativa, listo para codificar fuera del
/// hilo de audio (el remuestreo de 10 min no debe bloquear la captura).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Segment {
    /// WAV de 16 kHz mono.
    pub fn encode(&self) -> Result<Vec<u8>, AudioError> {
        encode_wav(&resample(&self.samples, self.sample_rate, TARGET_RATE))
    }
}

/// Acumula audio en mono a su frecuencia nativa. En grabaciones largas se van
/// sacando trozos (`take_chunk`) y solo se conserva lo que aún no se entregó.
#[derive(Debug)]
pub struct Recording {
    sample_rate: u32,
    channels: u16,
    max_frames: usize,
    /// Audio mono desde la trama absoluta `dropped`.
    samples: Vec<f32>,
    dropped: usize,
    /// Trama absoluta donde terminó el último trozo entregado.
    chunk_end: Option<usize>,
    overlap_frames: usize,
}

impl Recording {
    pub fn new(sample_rate: u32, channels: u16, max: Duration) -> Self {
        Self {
            sample_rate,
            channels: channels.max(1),
            max_frames: (max.as_secs_f64() * sample_rate as f64) as usize,
            samples: Vec::new(),
            dropped: 0,
            chunk_end: None,
            overlap_frames: 0,
        }
    }

    fn frames(&self) -> usize {
        self.dropped + self.samples.len()
    }

    fn to_frames(&self, d: Duration) -> usize {
        (d.as_secs_f64() * self.sample_rate as f64) as usize
    }

    /// Duración total grabada, incluidos los trozos ya entregados.
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames() as f64 / self.sample_rate as f64)
    }

    /// ¿Se ha entregado ya algún trozo? (grabación larga)
    pub fn is_chunked(&self) -> bool {
        self.chunk_end.is_some()
    }

    pub fn push(&mut self, interleaved: &[f32]) -> PushOutcome {
        let room = self.max_frames.saturating_sub(self.frames()) * self.channels as usize;
        let take = interleaved.len().min(room);
        self.samples
            .extend(to_mono(&interleaved[..take], self.channels));
        PushOutcome {
            rms: rms(interleaved),
            limit_reached: self.frames() >= self.max_frames,
        }
    }

    /// Si desde el último trozo hay al menos `len` de audio, devuelve el siguiente
    /// trozo (empieza `overlap` antes del final del anterior) y libera la memoria
    /// del audio que ya no hará falta.
    pub fn take_chunk(&mut self, len: Duration, overlap: Duration) -> Option<Segment> {
        let start = self.chunk_end.unwrap_or(0);
        let end = start + self.to_frames(len);
        if self.frames() < end {
            return None;
        }
        self.overlap_frames = self.to_frames(overlap);
        // El búfer empieza en `dropped`, que es el inicio del trozo (con solape).
        let end_rel = end - self.dropped;
        let keep_rel = end.saturating_sub(self.overlap_frames).max(self.dropped) - self.dropped;
        let tail = self.samples.split_off(keep_rel);
        let mut chunk = std::mem::replace(&mut self.samples, tail);
        chunk.extend_from_slice(&self.samples[..end_rel - keep_rel]);
        self.dropped += keep_rel;
        self.chunk_end = Some(end);
        Some(Segment {
            samples: chunk,
            sample_rate: self.sample_rate,
        })
    }

    /// El audio que queda: toda la grabación, o lo grabado desde el último trozo
    /// (con su solape). `None` si es demasiado corto.
    pub fn finish_segment(self) -> Option<Segment> {
        let new_frames = self.frames() - self.chunk_end.unwrap_or(0);
        if Duration::from_secs_f64(new_frames as f64 / self.sample_rate as f64) < MIN_DURATION {
            return None;
        }
        Some(Segment {
            samples: self.samples,
            sample_rate: self.sample_rate,
        })
    }

    /// WAV de 16 kHz mono, o `None` si la grabación es demasiado corta.
    pub fn finish(self) -> Result<Option<Vec<u8>>, AudioError> {
        self.finish_segment().map(|s| s.encode()).transpose()
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

    fn secs(wav: &[u8]) -> f64 {
        let r = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
        r.duration() as f64 / TARGET_RATE as f64
    }

    #[test]
    fn chunks_overlap_and_cover_the_whole_recording() {
        // 25 s a 48 kHz estéreo, trozos de 10 s con 1 s de solape.
        let mut r = Recording::new(48_000, 2, Duration::from_secs(60));
        let len = Duration::from_secs(10);
        let overlap = Duration::from_secs(1);
        r.push(&sine(48_000, 2, 9.0, 440.0, 0.5));
        assert!(r.take_chunk(len, overlap).is_none(), "aún no hay 10 s");
        r.push(&sine(48_000, 2, 16.0, 440.0, 0.5));
        let a = r.take_chunk(len, overlap).unwrap();
        let b = r.take_chunk(len, overlap).unwrap();
        assert!(r.take_chunk(len, overlap).is_none());
        assert!(r.is_chunked());
        assert_eq!(r.duration(), Duration::from_secs(25));
        // Primero 0–10 s; luego 9–20 s (1 s de solape); al final 19–25 s.
        assert!((secs(&a.encode().unwrap()) - 10.0).abs() < 0.01);
        assert!((secs(&b.encode().unwrap()) - 11.0).abs() < 0.01);
        let last = r.finish().unwrap().unwrap();
        assert!((secs(&last) - 6.0).abs() < 0.01);
    }

    #[test]
    fn taking_chunks_releases_memory() {
        let mut r = Recording::new(16_000, 1, Duration::from_secs(600));
        r.push(&sine(16_000, 1, 30.0, 440.0, 0.5));
        r.take_chunk(Duration::from_secs(10), Duration::from_secs(1));
        r.take_chunk(Duration::from_secs(10), Duration::from_secs(1));
        // Quedan 10 s nuevos más 1 s de solape, no los 30 s.
        assert_eq!(r.samples.len(), 11 * 16_000);
    }

    #[test]
    fn tiny_remainder_after_last_chunk_is_skipped() {
        let mut r = Recording::new(16_000, 1, Duration::from_secs(60));
        r.push(&sine(16_000, 1, 10.1, 440.0, 0.5));
        r.take_chunk(Duration::from_secs(10), Duration::from_secs(1))
            .unwrap();
        assert_eq!(r.finish().unwrap(), None);
    }

    #[test]
    fn chunked_audio_is_continuous() {
        // Una rampa: el solape del trozo 2 repite exactamente el final del 1.
        let ramp: Vec<f32> = (0..30_000).map(|i| i as f32 / 30_000.0).collect();
        let mut r = Recording::new(1_000, 1, Duration::from_secs(60));
        r.push(&ramp);
        let a = r
            .take_chunk(Duration::from_secs(10), Duration::from_secs(2))
            .unwrap();
        let b = r
            .take_chunk(Duration::from_secs(10), Duration::from_secs(2))
            .unwrap();
        assert_eq!(a.samples, ramp[..10_000]);
        assert_eq!(b.samples, ramp[8_000..20_000]);
        assert_eq!(r.finish_segment().unwrap().samples, ramp[18_000..]);
    }

    #[test]
    fn push_reports_block_rms() {
        let mut r = Recording::new(16_000, 1, DEFAULT_MAX_DURATION);
        assert!((r.push(&[0.5; 160]).rms - 0.5).abs() < 1e-6);
    }
}
