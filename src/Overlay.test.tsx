import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import Overlay, { formatElapsed } from "./Overlay";

type Handler = (e: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    handlers[name] = handler;
    return Promise.resolve(() => delete handlers[name]);
  }),
}));

describe("Overlay", () => {
  it("empieza en silencio y 0:00", () => {
    render(<Overlay />);
    expect(screen.getByTestId("level")).toHaveAttribute("data-active", "0");
    expect(screen.getByTestId("elapsed")).toHaveTextContent("0:00");
  });

  it("refleja el nivel y el tiempo de los eventos audio-level", () => {
    render(<Overlay />);
    act(() => handlers["audio-level"]({ payload: { level: 0.5, elapsed_ms: 65_400 } }));
    expect(screen.getByTestId("level")).toHaveAttribute("data-active", "6");
    expect(screen.getByTestId("elapsed")).toHaveTextContent("1:05");
    act(() => handlers["audio-level"]({ payload: { level: 1, elapsed_ms: 70_000 } }));
    expect(screen.getByTestId("level")).toHaveAttribute("data-active", "12");
  });

  it("formatea minutos y segundos", () => {
    expect(formatElapsed(0)).toBe("0:00");
    expect(formatElapsed(9_999)).toBe("0:09");
    expect(formatElapsed(300_000)).toBe("5:00");
  });
});
