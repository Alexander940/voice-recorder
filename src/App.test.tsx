import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as fx from "./harness/fixtures";
import { fire, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));
vi.mock("@tauri-apps/api/event", () => import("./test-tauri"));

const base = {
  get_key_status: { groq: true, anthropic: true },
  settings_get: fx.settings,
  modes_list: fx.modes,
  history_list: [],
  vocabulary_get: fx.vocabulary,
};

describe("App", () => {
  beforeEach(() => setCommands(base));

  it("abre en Historial con el menú lateral y la tarjeta de estado", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Historial" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Historial" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(await screen.findByText("Modo: Limpio")).toBeInTheDocument();
    expect(screen.getByText("Listo para dictar")).toBeInTheDocument();
    expect(screen.getByText("Option+Space")).toBeInTheDocument();
  });

  it("navega con el menú lateral y con el evento de la bandeja", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Vocabulario" }));
    expect(await screen.findByRole("heading", { name: "Vocabulario" })).toBeInTheDocument();
    act(() => fire("navigate", "settings"));
    expect(await screen.findByRole("heading", { name: "Ajustes" })).toBeInTheDocument();
  });

  it("la tarjeta de estado sigue a la bandeja", async () => {
    render(<App />);
    await screen.findByText("Modo: Limpio");
    act(() => fire("tray-state", "recording"));
    expect(screen.getByText("Grabando…")).toBeInTheDocument();
    act(() => fire("tray-state", "processing"));
    expect(screen.getByText("Procesando…")).toBeInTheDocument();
  });

  it("sin key de Groq abre el onboarding", async () => {
    setCommands({ ...base, get_key_status: { groq: false, anthropic: false } });
    render(<App />);
    expect(
      await screen.findByRole("heading", { name: "Bienvenido a Voice Recorder" }),
    ).toBeInTheDocument();
  });
});
