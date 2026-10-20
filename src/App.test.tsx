import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

type Handler = (e: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue("0.1.0"),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    handlers[name] = handler;
    return Promise.resolve(() => delete handlers[name]);
  }),
}));

describe("App", () => {
  beforeEach(() => {
    for (const k of Object.keys(handlers)) delete handlers[k];
  });

  it("muestra el título y la versión del backend", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Voice Recorder" })).toBeInTheDocument();
    expect(await screen.findByTestId("version")).toHaveTextContent("v0.1.0");
  });

  it("abre en Ajustes y navega a Historial con el evento de la bandeja", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Ajustes" })).toBeInTheDocument();
    await screen.findByTestId("version");
    act(() => handlers["navigate"]({ payload: "history" }));
    expect(screen.getByRole("heading", { name: "Historial" })).toBeInTheDocument();
  });
});
