//! Máquina de estados del atajo de dictado, independiente del SO.

use std::time::{Duration, Instant};

/// Por debajo de este tiempo, una pulsación se considera rebote o accidental.
pub const DEBOUNCE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    /// Graba mientras el atajo está pulsado.
    Hold,
    /// Una pulsación empieza y otra termina (por defecto).
    #[default]
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyInput {
    Pressed(Instant),
    Released(Instant),
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DictationAction {
    Start,
    Stop,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Recording { since: Instant },
}

#[derive(Debug)]
pub struct HotkeyController {
    mode: HotkeyMode,
    state: State,
}

impl HotkeyController {
    pub fn new(mode: HotkeyMode) -> Self {
        Self {
            mode,
            state: State::Idle,
        }
    }

    pub fn mode(&self) -> HotkeyMode {
        self.mode
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.state, State::Recording { .. })
    }

    /// Cambia el modo. Si hay una grabación en curso se cancela.
    pub fn set_mode(&mut self, mode: HotkeyMode) -> Option<DictationAction> {
        self.mode = mode;
        if self.is_recording() {
            self.state = State::Idle;
            Some(DictationAction::Cancel)
        } else {
            None
        }
    }

    /// Termina la grabación desde fuera (p. ej. límite de duración alcanzado).
    pub fn force_stop(&mut self) -> Option<DictationAction> {
        if self.is_recording() {
            self.state = State::Idle;
            Some(DictationAction::Stop)
        } else {
            None
        }
    }

    /// Empieza o termina desde la interfaz (menú de la bandeja), en cualquier modo.
    pub fn toggle_external(&mut self, now: Instant) -> DictationAction {
        if self.is_recording() {
            self.state = State::Idle;
            DictationAction::Stop
        } else {
            self.state = State::Recording { since: now };
            DictationAction::Start
        }
    }

    /// Cancela desde la interfaz (botón del overlay).
    pub fn cancel_external(&mut self) -> Option<DictationAction> {
        self.handle(HotkeyInput::Escape)
    }

    /// Vuelve a reposo sin emitir acción (p. ej. el micrófono no arrancó).
    pub fn reset(&mut self) {
        self.state = State::Idle;
    }

    pub fn handle(&mut self, input: HotkeyInput) -> Option<DictationAction> {
        use DictationAction::*;
        use HotkeyInput::*;
        match (self.mode, self.state, input) {
            (_, State::Recording { .. }, Escape) => {
                self.state = State::Idle;
                Some(Cancel)
            }
            (_, State::Idle, Pressed(t)) => {
                self.state = State::Recording { since: t };
                Some(Start)
            }
            // Mantener: soltar termina; una pulsación demasiado corta se descarta.
            (HotkeyMode::Hold, State::Recording { since }, Released(t)) => {
                self.state = State::Idle;
                if t.saturating_duration_since(since) < DEBOUNCE {
                    Some(Cancel)
                } else {
                    Some(Stop)
                }
            }
            // Alternar: una segunda pulsación termina, salvo que sea rebote.
            (HotkeyMode::Toggle, State::Recording { since }, Pressed(t)) => {
                if t.saturating_duration_since(since) < DEBOUNCE {
                    None
                } else {
                    self.state = State::Idle;
                    Some(Stop)
                }
            }
            // Repetición del SO con la tecla mantenida, soltar en Alternar, Esc o soltar en reposo.
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DictationAction::*;
    use super::HotkeyInput::*;
    use super::*;

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn external_toggle_starts_and_stops_in_both_modes() {
        let t = Instant::now();
        for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
            let mut c = HotkeyController::new(mode);
            assert_eq!(c.toggle_external(t), Start);
            assert!(c.is_recording());
            assert_eq!(c.toggle_external(ms(t, 10)), Stop);
            assert!(!c.is_recording());
        }
    }

    #[test]
    fn external_cancel_only_while_recording() {
        let t = Instant::now();
        let mut c = HotkeyController::new(HotkeyMode::Toggle);
        assert_eq!(c.cancel_external(), None);
        c.toggle_external(t);
        assert_eq!(c.cancel_external(), Some(Cancel));
        assert!(!c.is_recording());
    }

    #[test]
    fn default_mode_is_toggle() {
        assert_eq!(HotkeyMode::default(), HotkeyMode::Toggle);
        assert_eq!(
            HotkeyController::new(HotkeyMode::default()).mode(),
            HotkeyMode::Toggle
        );
    }

    #[test]
    fn hold_press_starts_release_stops() {
        let t = Instant::now();
        let mut c = HotkeyController::new(HotkeyMode::Hold);
        assert_eq!(c.handle(Pressed(t)), Some(Start));
        assert!(c.is_recording());
        assert_eq!(c.handle(Released(ms(t, 800))), Some(Stop));
        assert!(!c.is_recording());
    }

    #[test]
    fn hold_ignores_os_key_repeat() {
        let t = Instant::now();
        let mut c = HotkeyController::new(HotkeyMode::Hold);
        c.handle(Pressed(t));
        for i in 1..10 {
            assert_eq!(c.handle(Pressed(ms(t, i * 30))), None);
        }
        assert_eq!(c.handle(Released(ms(t, 400))), Some(Stop));
    }

    #[test]
    fn hold_tap_shorter_than_debounce_is_cancelled() {
        let t = Instant::now();
        let mut c = HotkeyController::new(HotkeyMode::Hold);
        c.handle(Pressed(t));
        assert_eq!(c.handle(Released(ms(t, 20))), Some(Cancel));
        assert!(!c.is_recording());
    }

    #[test]
    fn toggle_press_starts_second_press_stops_release_ignored() {
        let t = Instant::now();
        let mut c = HotkeyController::new(HotkeyMode::Toggle);
        assert_eq!(c.handle(Pressed(t)), Some(Start));
        assert_eq!(c.handle(Released(ms(t, 100))), None);
        assert!(c.is_recording());
        assert_eq!(c.handle(Pressed(ms(t, 3000))), Some(Stop));
        assert_eq!(c.handle(Released(ms(t, 3100))), None);
        assert!(!c.is_recording());
    }

    #[test]
    fn toggle_bounce_does_not_stop() {
        let t = Instant::now();
        let mut c = HotkeyController::new(HotkeyMode::Toggle);
        c.handle(Pressed(t));
        assert_eq!(c.handle(Pressed(ms(t, 10))), None);
        assert!(c.is_recording());
    }

    #[test]
    fn escape_cancels_in_both_modes_and_is_noop_when_idle() {
        for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
            let t = Instant::now();
            let mut c = HotkeyController::new(mode);
            assert_eq!(c.handle(Escape), None);
            c.handle(Pressed(t));
            assert_eq!(c.handle(Escape), Some(Cancel));
            assert!(!c.is_recording());
            // Soltar después de Esc no hace nada.
            assert_eq!(c.handle(Released(ms(t, 500))), None);
        }
    }

    #[test]
    fn release_while_idle_is_noop() {
        let mut c = HotkeyController::new(HotkeyMode::Hold);
        assert_eq!(c.handle(Released(Instant::now())), None);
    }

    #[test]
    fn force_stop_only_acts_while_recording() {
        let mut c = HotkeyController::new(HotkeyMode::Toggle);
        assert_eq!(c.force_stop(), None);
        c.handle(Pressed(Instant::now()));
        assert_eq!(c.force_stop(), Some(Stop));
        assert!(!c.is_recording());
    }

    #[test]
    fn reset_returns_to_idle_without_action() {
        let mut c = HotkeyController::new(HotkeyMode::Hold);
        c.handle(Pressed(Instant::now()));
        c.reset();
        assert!(!c.is_recording());
        assert_eq!(c.handle(Released(Instant::now())), None);
    }

    #[test]
    fn changing_mode_while_recording_cancels() {
        let mut c = HotkeyController::new(HotkeyMode::Hold);
        assert_eq!(c.set_mode(HotkeyMode::Toggle), None);
        c.handle(Pressed(Instant::now()));
        assert_eq!(c.set_mode(HotkeyMode::Hold), Some(Cancel));
        assert_eq!(c.mode(), HotkeyMode::Hold);
        assert!(!c.is_recording());
    }
}
