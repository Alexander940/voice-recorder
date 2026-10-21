import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Overlay, { barHeights, formatElapsed } from "./Overlay";
import { called, fire, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));
vi.mock("@tauri-apps/api/event", () => import("./test-tauri"));

describe("helpers", () => {
  it("formatea el tiempo", () => {
    expect(formatElapsed(0)).toBe("0:00");
    expect(formatElapsed(7_400)).toBe("0:07");
    expect(formatElapsed(65_000)).toBe("1:05");
    expect(formatElapsed(4_350_000)).toBe("1:12:30");
  });

  it("dibuja los niveles recientes y puntos donde aún no hay", () => {
    const h = barHeights([0.5, 1, 0, 2]);
    expect(h.slice(0, 4)).toEqual([11, 22, 4, 22]);
    expect(h.slice(4).every((x) => x === null)).toBe(true);
    expect(h).toHaveLength(12);
  });
});

describe("Overlay", () => {
  beforeEach(() => {
    setCommands({});
    globalThis.ResizeObserver ??= class {
      observe() {}
      disconnect() {}
      unobserve() {}
    } as unknown as typeof ResizeObserver;
  });

  it("no muestra nada hasta recibir un estado", () => {
    const { container } = render(<Overlay />);
    expect(container).toBeEmptyDOMElement();
  });

  it("grabando: modo, nivel, tiempo, ayuda y cancelar", () => {
    render(<Overlay />);
    act(() =>
      fire("overlay-state", {
        kind: "recording",
        mode: "Email",
        shortcut: "Option+Space",
        toggle: true,
      }),
    );
    act(() => fire("audio-level", { level: 0.5, elapsed_ms: 7_000 }));
    expect(screen.getByRole("status", { name: "Grabando" })).toBeInTheDocument();
    expect(screen.getByTestId("mode")).toHaveTextContent("Email");
    expect(screen.getByTestId("elapsed")).toHaveTextContent("0:07");
    expect(screen.getByTestId("level")).toHaveAttribute("data-active", "1");
    expect(screen.getByText(/para terminar · Esc para cancelar/)).toHaveTextContent(
      "Pulsa Option+Space para terminar",
    );
    fireEvent.click(screen.getByRole("button", { name: "Cancelar grabación" }));
    expect(called("dictation_cancel")).toHaveLength(1);
    expect(called("overlay_fit").length).toBeGreaterThan(0);
  });

  it("en un dictado largo muestra los trozos ya transcritos", () => {
    render(
      <Overlay initial={{ kind: "recording", mode: "Limpio", shortcut: "X", toggle: true }} />,
    );
    expect(screen.queryByTestId("chunks")).toBeNull();
    act(() => fire("dictation-progress", 1));
    expect(screen.getByTestId("chunks")).toHaveTextContent("1 trozo transcrito");
    act(() => fire("dictation-progress", 7));
    expect(screen.getByTestId("chunks")).toHaveTextContent("7 trozos transcritos");
  });

  it("en modo mantener la ayuda dice soltar", () => {
    render(
      <Overlay
        initial={{ kind: "recording", mode: "Limpio", shortcut: "Ctrl+Alt+Space", toggle: false }}
      />,
    );
    expect(screen.getByText(/para terminar/)).toHaveTextContent("Suelta Ctrl+Alt+Space");
  });

  it("procesando, pegado y error", () => {
    render(<Overlay />);
    act(() => fire("overlay-state", { kind: "processing", stage: "transcribing", mode: "Email" }));
    expect(screen.getByText("Transcribiendo…")).toBeInTheDocument();
    act(() => fire("overlay-state", { kind: "processing", stage: "cleaning", mode: "Email" }));
    expect(screen.getByText("Puliendo con Claude…")).toBeInTheDocument();
    act(() => fire("overlay-state", { kind: "pasted", words: 48, ms: 1_200, copied: false }));
    expect(screen.getByText("48 palabras · 1,2 s")).toBeInTheDocument();
    act(() =>
      fire("overlay-state", {
        kind: "error",
        title: "Sin conexión con Groq",
        detail: "El audio sigue guardado",
        retry: true,
      }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Sin conexión con Groq");
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    expect(called("dictation_retry")).toHaveLength(1);
  });

  it("un error sin reintento se puede cerrar", () => {
    render(
      <Overlay initial={{ kind: "error", title: "Sin micrófono", detail: "", retry: false }} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Cerrar" }));
    expect(called("overlay_dismiss")).toHaveLength(1);
  });
});
