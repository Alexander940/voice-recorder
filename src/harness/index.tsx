// Arnés de pantallas para el oráculo visual (solo en desarrollo, nunca en el bundle de
// producción). Simula el backend de Tauri con datos de ejemplo y deja cada pantalla en el
// estado que muestra su diseño. Renderiza siempre los componentes reales de la app.
import type { ReactElement } from "react";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "../App";
import Onboarding from "../Onboarding";
import Overlay from "../Overlay";
import * as fx from "./fixtures";

const COMMANDS: Record<string, unknown> = {
  app_version: "0.1.0",
  get_key_status: { groq: true, anthropic: false },
  get_key_hints: fx.keyHints,
  settings_get: fx.settings,
  list_input_devices: ["MacBook Pro Microphone"],
  modes_list: fx.modes,
  vocabulary_get: fx.vocabulary,
  history_list: fx.history,
  history_settings: { enabled: true, retention_days: 0 },
  dictation_retry_available: true,
  update_status: fx.update,
  platform: "macos",
};

interface Screen {
  render: () => ReactElement;
  /** Lleva la pantalla al estado del diseño (eventos simulados, clics). */
  drive?: () => Promise<void>;
}

const later = (ms = 50) => new Promise((r) => setTimeout(r, ms));

const SCREENS: Record<string, Screen> = {
  "onboarding-welcome": { render: () => <Onboarding onDone={() => {}} /> },
  "onboarding-groq": { render: () => <Onboarding onDone={() => {}} /> },
  "onboarding-permissions": { render: () => <Onboarding onDone={() => {}} /> },
  "onboarding-test": { render: () => <Onboarding onDone={() => {}} /> },
  history: {
    render: () => <App />,
    drive: async () => {
      await emit("navigate", "history");
    },
  },
  modes: { render: () => <App /> },
  vocabulary: { render: () => <App /> },
  settings: { render: () => <App /> },
  accounts: { render: () => <App /> },
  updates: { render: () => <App /> },
  "overlay-recording": { render: () => <Overlay /> },
  "overlay-processing": { render: () => <Overlay /> },
  "overlay-pasted": { render: () => <Overlay /> },
  "overlay-error": { render: () => <Overlay /> },
};

export function setupHarness(id: string): Screen {
  Object.defineProperty(navigator, "userAgent", {
    value: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Harness",
  });
  mockWindows(id.startsWith("overlay") ? "overlay" : "settings");
  mockIPC(
    (cmd) => {
      if (cmd in COMMANDS) return structuredClone(COMMANDS[cmd]);
      return null;
    },
    { shouldMockEvents: true },
  );
  const screen = SCREENS[id] ?? { render: () => <p>Pantalla desconocida: {id}</p> };
  return screen;
}

export async function finishHarness(screen: Screen) {
  await later();
  await screen.drive?.();
  await later(150);
  document.documentElement.dataset.harnessReady = "1";
}
