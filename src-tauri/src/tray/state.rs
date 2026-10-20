//! Máquina de estados del icono de la bandeja.

/// Estado visible de la app en la bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrayState {
    #[default]
    Idle,
    Recording,
    Processing,
}

/// Eventos del ciclo de dictado que mueven el estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    StartRecording,
    StopRecording,
    Cancel,
    ProcessingDone,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: TrayState,
    pub event: TrayEvent,
}

impl TrayState {
    /// Devuelve el siguiente estado, o `InvalidTransition` si el evento no aplica.
    /// `Error` siempre vuelve a `Idle` para que la app nunca quede atascada.
    pub fn next(self, event: TrayEvent) -> Result<TrayState, InvalidTransition> {
        use TrayEvent::*;
        use TrayState::*;
        match (self, event) {
            (_, Error) => Ok(Idle),
            (Idle, StartRecording) => Ok(Recording),
            (Recording, StopRecording) => Ok(Processing),
            (Recording, Cancel) => Ok(Idle),
            (Processing, ProcessingDone) => Ok(Idle),
            (Processing, Cancel) => Ok(Idle),
            (from, event) => Err(InvalidTransition { from, event }),
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            TrayState::Idle => "Voice Recorder — listo",
            TrayState::Recording => "Voice Recorder — grabando…",
            TrayState::Processing => "Voice Recorder — procesando…",
        }
    }

    /// Color RGB del icono para el estado.
    pub fn color(self) -> [u8; 3] {
        match self {
            TrayState::Idle => [0x8a, 0x8f, 0x98],
            TrayState::Recording => [0xe5, 0x3e, 0x3e],
            TrayState::Processing => [0xf0, 0xa2, 0x02],
        }
    }
}

/// Icono RGBA cuadrado: un círculo del color del estado sobre fondo transparente.
pub fn icon_rgba(state: TrayState, size: u32) -> Vec<u8> {
    let [r, g, b] = state.color();
    let c = (size as f32 - 1.0) / 2.0;
    let radius = size as f32 / 2.0 - 1.0;
    let mut buf = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            // Borde suavizado de 1 px.
            let alpha = (radius + 0.5 - d).clamp(0.0, 1.0);
            buf.extend_from_slice(&[r, g, b, (alpha * 255.0) as u8]);
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::TrayEvent::*;
    use super::TrayState::*;
    use super::*;

    #[test]
    fn happy_path_idle_recording_processing_idle() {
        let s = Idle.next(StartRecording).unwrap();
        assert_eq!(s, Recording);
        let s = s.next(StopRecording).unwrap();
        assert_eq!(s, Processing);
        assert_eq!(s.next(ProcessingDone).unwrap(), Idle);
    }

    #[test]
    fn cancel_returns_to_idle() {
        assert_eq!(Recording.next(Cancel).unwrap(), Idle);
        assert_eq!(Processing.next(Cancel).unwrap(), Idle);
    }

    #[test]
    fn error_always_returns_to_idle() {
        for s in [Idle, Recording, Processing] {
            assert_eq!(s.next(Error).unwrap(), Idle);
        }
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        for (from, event) in [
            (Idle, StopRecording),
            (Idle, Cancel),
            (Idle, ProcessingDone),
            (Recording, StartRecording),
            (Recording, ProcessingDone),
            (Processing, StartRecording),
            (Processing, StopRecording),
        ] {
            assert_eq!(from.next(event), Err(InvalidTransition { from, event }));
        }
    }

    #[test]
    fn default_is_idle() {
        assert_eq!(TrayState::default(), Idle);
    }

    #[test]
    fn each_state_has_distinct_color_and_tooltip() {
        let states = [Idle, Recording, Processing];
        for (i, a) in states.iter().enumerate() {
            for b in &states[i + 1..] {
                assert_ne!(a.color(), b.color());
                assert_ne!(a.tooltip(), b.tooltip());
            }
        }
    }

    #[test]
    fn icon_has_rgba_size_and_transparent_corners() {
        let size = 32;
        let px = icon_rgba(Recording, size);
        assert_eq!(px.len(), (size * size * 4) as usize);
        assert_eq!(px[3], 0, "la esquina debe ser transparente");
        let center = (((size / 2) * size + size / 2) * 4) as usize;
        assert_eq!(&px[center..center + 4], &[0xe5, 0x3e, 0x3e, 255]);
    }
}
