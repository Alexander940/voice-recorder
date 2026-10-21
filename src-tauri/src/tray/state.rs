//! Máquina de estados del icono de la bandeja.

/// Estado visible de la app en la bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "lowercase")]
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

    /// Primera línea del menú de la bandeja.
    pub fn status_text(self) -> &'static str {
        match self {
            TrayState::Idle => "Listo para dictar",
            TrayState::Recording => "Grabando…",
            TrayState::Processing => "Procesando…",
        }
    }

    /// Color RGB del icono para el estado (docs/design/tray-spec.json).
    pub fn color(self) -> [u8; 3] {
        match self {
            TrayState::Idle => [0x6b, 0x71, 0x7c],
            TrayState::Recording => [0xd9, 0x2d, 0x2d],
            TrayState::Processing => [0x4f, 0x46, 0xe5],
        }
    }
}

/// Distancia de un punto a un segmento.
fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

/// Icono RGBA cuadrado: un micrófono del color del estado sobre fondo transparente.
/// Se dibuja en una rejilla de 24 unidades (como los iconos de la interfaz).
pub fn icon_rgba(state: TrayState, size: u32) -> Vec<u8> {
    let [r, g, b] = state.color();
    let unit = size as f32 / 24.0;
    let stroke = 1.1;
    let mut buf = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let p = ((x as f32 + 0.5) / unit, (y as f32 + 0.5) / unit);
            // Cápsula rellena (x 9–15, y 3–14) y, en Grabando, también su interior.
            let capsule = segment_distance(p, (12.0, 6.0), (12.0, 11.0)) - 3.0;
            // Arco inferior de radio 7 centrado en (12, 11).
            let d = ((p.0 - 12.0).powi(2) + (p.1 - 11.0).powi(2)).sqrt();
            let arc = if p.1 >= 11.0 {
                (d - 7.0).abs() - stroke
            } else {
                f32::MAX
            };
            let stem = segment_distance(p, (12.0, 18.0), (12.0, 21.0)) - stroke;
            let shape = capsule.min(arc).min(stem);
            // Borde suavizado de 1 px.
            let alpha = (0.5 - shape * unit).clamp(0.0, 1.0);
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
        // El centro de la cápsula del micrófono es opaco y del color del estado.
        let at = |x: u32, y: u32| ((y * size + x) * 4) as usize;
        let c = at(16, 11);
        assert_eq!(&px[c..c + 4], &[0xd9, 0x2d, 0x2d, 255]);
        // Entre la cápsula y el arco hay hueco: no es un círculo relleno.
        let gap = at(16, 19);
        assert_eq!(px[gap + 3], 0);
    }

    /// Los colores salen del diseño aprobado (docs/design/tray-spec.json).
    #[test]
    fn colors_and_status_texts_match_the_design_spec() {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/design/tray-spec.json")).unwrap();
        for (state, key) in [
            (Idle, "idle"),
            (Recording, "recording"),
            (Processing, "processing"),
        ] {
            let [r, g, b] = state.color();
            assert_eq!(
                format!("#{r:02X}{g:02X}{b:02X}"),
                spec["icon_colors"][key].as_str().unwrap()
            );
            assert_eq!(
                state.status_text(),
                spec["status_text"][key].as_str().unwrap()
            );
        }
    }
}
