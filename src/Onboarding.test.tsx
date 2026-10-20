import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Onboarding from "./Onboarding";

type Handler = (e: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};
const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    handlers[name] = handler;
    return Promise.resolve(() => delete handlers[name]);
  }),
}));

const click = (name: RegExp) => fireEvent.click(screen.getByRole("button", { name }));

async function saveKey(value: string) {
  fireEvent.change(screen.getByLabelText("API key"), { target: { value } });
  await act(async () => click(/validar y guardar/i));
}

describe("Onboarding", () => {
  beforeEach(() => {
    invoke.mockReset();
    for (const k of Object.keys(handlers)) delete handlers[k];
  });

  it("paso 1: bienvenida → Groq", () => {
    render(<Onboarding onDone={() => {}} />);
    expect(screen.getByText("Paso 1 de 5")).toBeInTheDocument();
    click(/empezar/i);
    expect(screen.getByRole("heading", { name: "API key de Groq" })).toBeInTheDocument();
  });

  it("paso 2: Groq exige key y envía provider + key al backend", async () => {
    invoke.mockResolvedValue({ groq: true, anthropic: false });
    render(<Onboarding onDone={() => {}} />);
    click(/empezar/i);
    expect(screen.getByRole("button", { name: /validar y guardar/i })).toBeDisabled();
    await saveKey("gsk_abc");
    expect(invoke).toHaveBeenCalledWith("save_api_key", { provider: "groq", key: "gsk_abc" });
    expect(screen.getByRole("heading", { name: "API key de Anthropic" })).toBeInTheDocument();
  });

  it("paso 2: muestra el error del backend y no avanza", async () => {
    invoke.mockRejectedValue("No parece una key de Groq: debe empezar por «gsk_»");
    render(<Onboarding onDone={() => {}} />);
    click(/empezar/i);
    await saveKey("xai-123");
    expect(screen.getByRole("alert")).toHaveTextContent("gsk_");
    expect(screen.getByRole("heading", { name: "API key de Groq" })).toBeInTheDocument();
    expect(screen.getByLabelText("API key")).toHaveValue("xai-123");
  });

  it("paso 3: Anthropic se puede guardar u omitir", async () => {
    invoke.mockResolvedValue({ groq: true, anthropic: true });
    render(<Onboarding onDone={() => {}} />);
    click(/empezar/i);
    await saveKey("gsk_abc");
    click(/omitir por ahora/i);
    expect(screen.getByRole("heading", { name: "Permisos" })).toBeInTheDocument();
  });

  it("pasos 4 y 5: permisos → prueba, muestra la transcripción y termina", async () => {
    invoke.mockResolvedValue({ groq: true, anthropic: false });
    const onDone = vi.fn();
    render(<Onboarding onDone={onDone} />);
    click(/empezar/i);
    await saveKey("gsk_abc");
    click(/omitir por ahora/i);
    click(/continuar/i);
    expect(screen.getByRole("heading", { name: "Prueba de dictado" })).toBeInTheDocument();
    await act(async () => {});
    act(() => handlers["transcription"]({ payload: "hola mundo" }));
    expect(screen.getByTestId("transcript")).toHaveTextContent("hola mundo");
    click(/terminar/i);
    expect(onDone).toHaveBeenCalled();
  });

  it("la key no queda en el input tras guardarla", async () => {
    invoke.mockResolvedValue({ groq: true, anthropic: false });
    render(<Onboarding onDone={() => {}} />);
    click(/empezar/i);
    await saveKey("gsk_secreta");
    expect(screen.queryByDisplayValue("gsk_secreta")).not.toBeInTheDocument();
  });
});
