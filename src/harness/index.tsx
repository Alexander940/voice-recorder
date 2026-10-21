// Arnés de pantallas para el oráculo visual (solo en desarrollo, nunca en el bundle de
// producción). Simula el backend de Tauri con datos de ejemplo y deja cada pantalla en el
// estado que muestra su diseño. Renderiza siempre los componentes reales de la app y solo
// interactúa con ellos como lo haría una persona (teclear, pulsar) o el backend (eventos).
import type { ReactElement } from "react";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App, { type View } from "../App";
import Onboarding, { type Step } from "../Onboarding";
import Overlay, { type OverlayState } from "../Overlay";
import * as fx from "./fixtures";

type Handler = (args: Record<string, unknown>) => unknown;

function commands(overrides: Record<string, unknown> = {}): Record<string, unknown | Handler> {
  return {
    app_version: "0.1.0",
    get_key_status: { groq: true, anthropic: false },
    get_key_hints: fx.keyHints,
    settings_get: fx.settings,
    list_input_devices: ["MacBook Pro Microphone"],
    modes_list: fx.modes,
    modes_set_active: (a: Record<string, unknown>) => ({ ...fx.modes, active: a.id }),
    vocabulary_get: fx.vocabulary,
    vocabulary_save: (a: Record<string, unknown>) => a.vocabulary,
    history_list: fx.history,
    history_settings: { enabled: true, retention_days: 0 },
    dictation_retry_available: true,
    update_status: fx.update,
    save_api_key: () => {
      // El backend rechaza una key demasiado corta (KeyError::WrongFormat).
      throw "No parece una key de Groq: debe empezar por «gsk_»";
    },
    ...overrides,
  };
}

interface Screen {
  render: () => ReactElement;
  commands?: Record<string, unknown>;
  /** Lleva la pantalla al estado del diseño (eventos simulados, clics, teclado). */
  drive?: () => Promise<void>;
}

const later = (ms = 50) => new Promise((r) => setTimeout(r, ms));

function find<T extends Element>(selector: string, text?: string): T {
  const all = Array.from(document.querySelectorAll<T>(selector));
  const el = text ? all.find((e) => e.textContent?.trim() === text) : all[0];
  if (!el) throw new Error(`arnés: no encuentro ${selector} ${text ?? ""}`);
  return el;
}

/** Escribe en un campo como lo haría React con una pulsación real. */
function type(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

const app = (view: View) => () => <App initialView={view} />;
const onboarding = (step: Step) => () => <Onboarding onDone={() => {}} initialStep={step} />;

const recording: OverlayState = {
  kind: "recording",
  mode: "Email",
  shortcut: "Option+Space",
  toggle: true,
};

const SCREENS: Record<string, Screen> = {
  "onboarding-welcome": { render: onboarding("welcome") },
  "onboarding-groq": {
    render: onboarding("groq"),
    drive: async () => {
      type(find<HTMLInputElement>("#groq-key"), "gsk_xxxxxxxxxxxxxxxxxxxx");
      await later();
      find<HTMLButtonElement>("button", "Validar y guardar").click();
    },
  },
  "onboarding-permissions": { render: onboarding("permissions") },
  "onboarding-test": {
    render: onboarding("test"),
    drive: async () => {
      await emit("dictation-result", fx.dictationResult);
    },
  },
  history: { render: app("history") },
  modes: {
    render: app("modes"),
    commands: { modes_list: { ...fx.modes, active: "email" } },
  },
  vocabulary: {
    render: app("vocabulary"),
    drive: async () => {
      find<HTMLButtonElement>("button", "Guardar vocabulario").click();
    },
  },
  settings: {
    render: app("settings"),
    drive: async () => {
      type(find<HTMLInputElement>('input[type="number"][min="0"]'), "30");
    },
  },
  accounts: { render: app("accounts") },
  updates: { render: app("updates") },
  "overlay-recording": {
    render: () => <Overlay initial={recording} />,
    drive: async () => {
      for (const h of [6, 12, 18, 22, 14, 20, 10]) {
        await emit("audio-level", { level: h / 22, elapsed_ms: 7_000 });
      }
    },
  },
  "overlay-processing": {
    render: () => <Overlay initial={{ kind: "processing", stage: "cleaning", mode: "Email" }} />,
  },
  "overlay-pasted": {
    render: () => <Overlay initial={{ kind: "pasted", words: 48, ms: 1_200, copied: false }} />,
  },
  "overlay-error": {
    render: () => (
      <Overlay
        initial={{
          kind: "error",
          title: "Sin conexión con Groq",
          detail: "El audio sigue guardado",
          retry: true,
        }}
      />
    ),
  },
};

export function setupHarness(id: string): Screen {
  Object.defineProperty(navigator, "userAgent", {
    value: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Harness",
  });
  mockWindows(id.startsWith("overlay") ? "overlay" : "settings");
  const screen = SCREENS[id] ?? { render: () => <p>Pantalla desconocida: {id}</p> };
  const table = commands(screen.commands);
  mockIPC(
    (cmd, payload) => {
      const value = table[cmd];
      if (typeof value === "function")
        return (value as Handler)(payload as Record<string, unknown>);
      return value === undefined ? null : structuredClone(value);
    },
    { shouldMockEvents: true },
  );
  return screen;
}

export async function finishHarness(screen: Screen) {
  await later(100);
  await screen.drive?.();
  await later(200);
  document.documentElement.dataset.harnessReady = "1";
}
