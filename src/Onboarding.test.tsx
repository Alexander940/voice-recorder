import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Onboarding from "./Onboarding";
import * as fx from "./harness/fixtures";
import { called, fire, Reject, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));
vi.mock("@tauri-apps/api/event", () => import("./test-tauri"));

const next = (name: string) => fireEvent.click(screen.getByRole("button", { name }));

describe("Onboarding", () => {
  beforeEach(() => setCommands({ save_api_key: { groq: true, anthropic: false } }));

  it("recorre los cinco pasos", async () => {
    const onDone = vi.fn();
    render(<Onboarding onDone={onDone} />);
    expect(screen.getByText("Paso 1 de 5")).toBeInTheDocument();
    expect(screen.getByText("3. Púlsalo otra vez")).toBeInTheDocument();
    next("Empezar");

    expect(screen.getByRole("heading", { name: "API key de Groq" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "gsk_valida" } });
    next("Validar y guardar");
    expect(
      await screen.findByRole("heading", { name: "API key de Anthropic" }),
    ).toBeInTheDocument();
    expect(called("save_api_key")[0].args).toEqual({ provider: "groq", key: "gsk_valida" });
    // La key no se arrastra al paso siguiente.
    expect(screen.getByLabelText("API key")).toHaveValue("");

    next("Omitir por ahora");
    expect(screen.getByText("Paso 4 de 5")).toBeInTheDocument();
    next("Continuar");
    expect(screen.getByRole("heading", { name: "Prueba tu primer dictado" })).toBeInTheDocument();
    next("Saltar prueba");
    expect(onDone).toHaveBeenCalled();
  });

  it("Atrás vuelve al paso anterior", () => {
    render(<Onboarding onDone={() => {}} initialStep="anthropic" />);
    next("Atrás");
    expect(screen.getByRole("heading", { name: "API key de Groq" })).toBeInTheDocument();
  });

  it("explica una key con formato incorrecto", async () => {
    setCommands({
      save_api_key: () => new Reject("No parece una key de Groq: debe empezar por «gsk_»"),
    });
    render(<Onboarding onDone={() => {}} initialStep="groq" />);
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "xai-1" } });
    next("Validar y guardar");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Esta key no parece de Groq: debe empezar por gsk_. Las keys de xAI (Grok) no sirven.",
    );
  });

  it("muestra el resultado de la prueba de dictado", () => {
    render(<Onboarding onDone={() => {}} initialStep="test" />);
    expect(screen.getByText("Esperando tu dictado…")).toBeInTheDocument();
    act(() => fire("dictation-result", fx.dictationResult));
    expect(screen.getByTestId("transcript")).toHaveTextContent("Hola, esto es una prueba.");
    expect(screen.getByText("Modo Limpio")).toBeInTheDocument();
    expect(screen.getByText("4,2 s de audio · procesado en 1,1 s")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Terminar" })).toBeInTheDocument();
  });

  it("en macOS guía los permisos y abre Accesibilidad", () => {
    const ua = vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Macintosh");
    render(<Onboarding onDone={() => {}} initialStep="permissions" />);
    expect(screen.getByRole("heading", { name: "Dos permisos y listo" })).toBeInTheDocument();
    next("Abrir Ajustes");
    expect(called("open_link")[0].args).toEqual({ target: "accessibility" });
    ua.mockRestore();
  });

  it("fuera de macOS no pide permisos", () => {
    render(<Onboarding onDone={() => {}} initialStep="permissions" />);
    expect(screen.getByRole("heading", { name: "Sin permisos adicionales" })).toBeInTheDocument();
  });
});
