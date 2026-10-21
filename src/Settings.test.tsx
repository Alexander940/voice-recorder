import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Settings, { shortcutFromEvent, type AppSettings } from "./Settings";
import * as fx from "./harness/fixtures";
import { called, Reject, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));

const settings = fx.settings as AppSettings;

describe("shortcutFromEvent", () => {
  const ev = (o: Partial<KeyboardEvent>) => ({
    key: "",
    code: "",
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...o,
  });

  it("usa los nombres del backend en cada sistema", () => {
    expect(shortcutFromEvent(ev({ key: " ", code: "Space", altKey: true }), true)).toBe(
      "Option+Space",
    );
    expect(
      shortcutFromEvent(ev({ key: "k", code: "KeyK", ctrlKey: true, altKey: true }), false),
    ).toBe("Ctrl+Alt+K");
    expect(shortcutFromEvent(ev({ key: "1", code: "Digit1", metaKey: true }), true)).toBe("Cmd+1");
  });

  it("ignora teclas sueltas y modificadores solos", () => {
    expect(shortcutFromEvent(ev({ key: "a", code: "KeyA" }), false)).toBeNull();
    expect(
      shortcutFromEvent(ev({ key: "Control", code: "ControlLeft", ctrlKey: true })),
    ).toBeNull();
  });
});

describe("Settings", () => {
  beforeEach(() =>
    setCommands({
      settings_get: settings,
      list_input_devices: ["USB Mic"],
      settings_update: (a: Record<string, unknown>) => a.settings,
    }),
  );

  it("muestra los valores guardados", async () => {
    render(<Settings />);
    expect(await screen.findByRole("heading", { name: "Ajustes" })).toBeInTheDocument();
    expect(screen.getByText("Option")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Pulsar para empezar y terminar" })).toBeChecked();
    expect(screen.getByLabelText("Micrófono")).toHaveValue("");
    expect(screen.getByRole("option", { name: "USB Mic" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: /Limpiar el texto con Claude/ })).toBeChecked();
    expect(screen.getByRole("switch", { name: /Solo copiar/ })).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Guardar ajustes" })).toBeDisabled();
  });

  it("avisa de cambios sin guardar, descarta y guarda", async () => {
    const onSaved = vi.fn();
    render(<Settings onSaved={onSaved} />);
    fireEvent.click(await screen.findByRole("radio", { name: "Mantener pulsado" }));
    expect(screen.getByText("Hay cambios sin guardar")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Descartar" }));
    expect(screen.queryByText("Hay cambios sin guardar")).toBeNull();

    fireEvent.click(screen.getByRole("switch", { name: /Sonidos/ }));
    fireEvent.click(screen.getByRole("button", { name: "Guardar ajustes" }));
    expect(await screen.findByText("Ajustes guardados")).toBeInTheDocument();
    const sent = called("settings_update")[0].args!.settings as AppSettings;
    expect(sent.feedback.sounds).toBe(false);
    expect(sent.hotkey.mode).toBe("toggle");
    expect(onSaved).toHaveBeenCalledWith(sent);
  });

  it("captura un atajo nuevo con el teclado", async () => {
    render(<Settings />);
    const button = await screen.findByRole("button", { name: "Cambiar" });
    fireEvent.click(button);
    expect(screen.getByText("Pulsa la combinación…")).toBeInTheDocument();
    fireEvent.keyDown(button, { key: "k", code: "KeyK", ctrlKey: true, altKey: true });
    expect(screen.getByText("K")).toBeInTheDocument();
    expect(screen.getByText("Hay cambios sin guardar")).toBeInTheDocument();
  });

  it("permite hasta 3 h y avisa del límite de Groq a partir de 2 h", async () => {
    render(<Settings />);
    const minutes = await screen.findByLabelText(/Duración máxima/);
    expect(minutes).toHaveAttribute("max", "180");
    expect(screen.queryByText(/agotar tu límite/)).toBeNull();
    fireEvent.change(minutes, { target: { value: "150" } });
    expect(screen.getByText(/agotar tu límite de audio por hora de Groq/)).toBeInTheDocument();
  });

  it("la retención se desactiva sin historial", async () => {
    render(<Settings />);
    fireEvent.click(await screen.findByRole("switch", { name: /Guardar historial/ }));
    expect(screen.getByLabelText(/Conservar el historial/)).toBeDisabled();
  });

  it("muestra el error del backend al guardar", async () => {
    setCommands({
      settings_get: settings,
      list_input_devices: [],
      settings_update: () => new Reject("El atajo ya está en uso"),
    });
    render(<Settings />);
    fireEvent.click(await screen.findByRole("switch", { name: /Sonidos/ }));
    fireEvent.click(screen.getByRole("button", { name: "Guardar ajustes" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("El atajo ya está en uso");
  });
});
