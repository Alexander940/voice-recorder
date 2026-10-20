import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Settings, { type AppSettings } from "./Settings";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

// La sección de actualizaciones escucha eventos del backend.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const DEFAULTS: AppSettings = {
  version: 1,
  general: { autostart: false },
  hotkey: { shortcut: "Ctrl+Alt+Space", mode: "hold" },
  audio: { device: null, max_duration_secs: 300 },
  transcription: { model: "whisper-large-v3-turbo", language: null },
  ai: { cleanup: true },
  output: { copy_only: false },
  privacy: { enabled: true, retention_days: 30 },
  feedback: { sounds: true },
  updates: { enabled: true },
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
    switch (cmd) {
      case "settings_get":
        return Promise.resolve(DEFAULTS);
      case "settings_update":
        return Promise.resolve(args.settings);
      case "list_input_devices":
        return Promise.resolve(["Micrófono USB", "Auriculares"]);
      case "get_key_status":
        return Promise.resolve({ groq: true, anthropic: false });
      case "delete_api_key":
        return Promise.resolve({ groq: false, anthropic: false });
      case "modes_list":
        return Promise.resolve({ modes: [], active: "clean" });
      case "update_status":
        return Promise.resolve({ configured: false, enabled: true, available: null });
      case "vocabulary_get":
        return Promise.resolve({ terms: [], replacements: [] });
      default:
        return Promise.resolve(null);
    }
  });
});

async function renderSettings() {
  render(<Settings />);
  await screen.findByRole("heading", { name: "General" });
  await act(async () => {});
}

const lastUpdate = () => {
  const calls = invoke.mock.calls.filter(([c]) => c === "settings_update");
  return calls[calls.length - 1][1].settings as AppSettings;
};

describe("Settings", () => {
  it("muestra todas las secciones", async () => {
    await renderSettings();
    for (const name of [
      "General",
      "Atajos",
      "Audio",
      "Transcripción",
      "IA",
      "Cuentas",
      "Privacidad",
      "Actualizaciones",
    ]) {
      expect(screen.getByRole("heading", { name })).toBeInTheDocument();
    }
  });

  it("General: inicio con el sistema y sonidos", async () => {
    await renderSettings();
    fireEvent.click(screen.getByLabelText("Iniciar con el sistema"));
    fireEvent.click(screen.getByLabelText("Sonidos al empezar, terminar y en errores"));
    await act(async () => fireEvent.click(screen.getByText("Guardar ajustes")));
    expect(lastUpdate().general.autostart).toBe(true);
    expect(lastUpdate().feedback.sounds).toBe(false);
    expect(screen.getByRole("status")).toHaveTextContent("Ajustes guardados");
  });

  it("Atajos: cambia atajo y modo; muestra el conflicto del backend", async () => {
    await renderSettings();
    fireEvent.change(screen.getByLabelText("Atajo de dictado"), {
      target: { value: "Ctrl+Shift+D" },
    });
    fireEvent.change(screen.getByLabelText("Modo del atajo"), { target: { value: "toggle" } });
    invoke.mockRejectedValueOnce("El atajo «Ctrl+Shift+D» ya está en uso por otra aplicación");
    await act(async () => fireEvent.click(screen.getByText("Guardar ajustes")));
    expect(lastUpdate().hotkey).toEqual({ shortcut: "Ctrl+Shift+D", mode: "toggle" });
    expect(screen.getByRole("alert")).toHaveTextContent("ya está en uso");
  });

  it("Audio: lista micrófonos y guarda dispositivo y duración", async () => {
    await renderSettings();
    const select = screen.getByLabelText("Micrófono");
    expect(within(select).getAllByRole("option")).toHaveLength(3);
    fireEvent.change(select, { target: { value: "Auriculares" } });
    fireEvent.change(screen.getByLabelText("Duración máxima (minutos)"), {
      target: { value: "10" },
    });
    await act(async () => fireEvent.click(screen.getByText("Guardar ajustes")));
    expect(lastUpdate().audio).toEqual({ device: "Auriculares", max_duration_secs: 600 });
  });

  it("Transcripción: modelo e idioma (vacío = automático)", async () => {
    await renderSettings();
    fireEvent.change(screen.getByLabelText("Modelo"), { target: { value: "whisper-large-v3" } });
    fireEvent.change(screen.getByLabelText("Idioma"), { target: { value: "es" } });
    await act(async () => fireEvent.click(screen.getByText("Guardar ajustes")));
    expect(lastUpdate().transcription).toEqual({ model: "whisper-large-v3", language: "es" });
  });

  it("IA: limpieza y solo copiar", async () => {
    await renderSettings();
    fireEvent.click(screen.getByLabelText("Limpiar el texto con Claude"));
    fireEvent.click(screen.getByLabelText("Solo copiar al portapapeles (no pegar)"));
    await act(async () => fireEvent.click(screen.getByText("Guardar ajustes")));
    expect(lastUpdate().ai.cleanup).toBe(false);
    expect(lastUpdate().output.copy_only).toBe(true);
  });

  it("Cuentas: estado de las keys y borrar", async () => {
    await renderSettings();
    const list = screen.getByRole("list", { name: "API keys" });
    expect(list).toHaveTextContent("Groq: configurada");
    expect(list).toHaveTextContent("Anthropic: sin configurar");
    await act(async () => fireEvent.click(within(list).getByText("Borrar")));
    expect(invoke).toHaveBeenCalledWith("delete_api_key", { provider: "groq" });
    expect(list).toHaveTextContent("Groq: sin configurar");
  });

  it("Cuentas: añadir abre el formulario de key", async () => {
    await renderSettings();
    fireEvent.click(screen.getByText("Añadir"));
    expect(screen.getByRole("heading", { name: "API key de Anthropic" })).toBeInTheDocument();
  });

  it("Privacidad: desactivar historial bloquea la retención", async () => {
    await renderSettings();
    fireEvent.click(screen.getByLabelText("Guardar historial de dictados"));
    expect(screen.getByLabelText("Conservar (días, 0 = siempre)")).toBeDisabled();
    await act(async () => fireEvent.click(screen.getByText("Guardar ajustes")));
    expect(lastUpdate().privacy).toEqual({ enabled: false, retention_days: 30 });
  });

  it("Actualizaciones: el interruptor se guarda con los ajustes", async () => {
    await renderSettings();
    fireEvent.click(screen.getByLabelText("Buscar actualizaciones al iniciar y una vez al día"));
    await act(async () => fireEvent.click(screen.getByText("Guardar ajustes")));
    expect(lastUpdate().updates.enabled).toBe(false);
  });
});
