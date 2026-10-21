import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import History, { dayLabel, groupByDay } from "./History";
import * as fx from "./harness/fixtures";
import { called, fire, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));
vi.mock("@tauri-apps/api/event", () => import("./test-tauri"));

describe("dayLabel / groupByDay", () => {
  const now = new Date(2026, 9, 21, 12).getTime();

  it("hoy, ayer y fechas", () => {
    expect(dayLabel(new Date(2026, 9, 21, 8).getTime(), now)).toBe("Hoy");
    expect(dayLabel(new Date(2026, 9, 20, 23).getTime(), now)).toBe("Ayer");
    expect(dayLabel(new Date(2026, 9, 15, 9).getTime(), now)).toMatch(/15/);
  });

  it("agrupa manteniendo el orden", () => {
    const groups = groupByDay(fx.history);
    expect(groups.map((g) => [g.label, g.entries.length])).toEqual([
      ["Hoy", 3],
      ["Ayer", 1],
    ]);
  });
});

describe("History", () => {
  beforeEach(() =>
    setCommands({
      history_list: fx.history,
      dictation_retry_available: false,
    }),
  );

  it("agrupa por día con el modo y destaca el último dictado", async () => {
    render(<History modes={fx.modes} />);
    expect(await screen.findByText("Hoy")).toBeInTheDocument();
    expect(screen.getByText("Ayer")).toBeInTheDocument();
    const latest = screen.getByText(/Hola Marta/).closest("li")!;
    expect(latest).toHaveClass("latest");
    expect(within(latest).getByText("Email")).toHaveClass("chip", "accent");
    expect(within(latest).getByText("12,8 s")).toBeInTheDocument();
  });

  it("muestra el código entre comillas invertidas como código", async () => {
    render(<History modes={fx.modes} />);
    expect(await screen.findByText("run_pipeline")).toHaveClass("code");
  });

  it("copiar, pegar de nuevo y borrar llaman al backend", async () => {
    render(<History modes={fx.modes} />);
    const latest = (await screen.findByText(/Hola Marta/)).closest("li")!;
    fireEvent.click(within(latest).getByText("Copiar"));
    fireEvent.click(within(latest).getByText("Pegar de nuevo"));
    fireEvent.click(within(latest).getByRole("button", { name: "Borrar" }));
    await screen.findByText("Borrado");
    expect(called("history_deliver").map((c) => c.args)).toEqual([
      { id: 4, paste: false },
      { id: 4, paste: true },
    ]);
    expect(called("history_delete")[0].args).toEqual({ id: 4 });
  });

  it("busca con el texto escrito", async () => {
    render(<History modes={fx.modes} />);
    await screen.findByText("Hoy");
    fireEvent.change(screen.getByLabelText("Buscar en el historial"), {
      target: { value: "marta" },
    });
    await vi.waitFor(() =>
      expect(called("history_list").slice(-1)[0]?.args).toEqual({ query: "marta" }),
    );
  });

  it("sin dictados muestra un estado vacío", async () => {
    setCommands({ history_list: [], dictation_retry_available: false });
    render(<History modes={fx.modes} />);
    expect(await screen.findByText("Todavía no hay dictados.")).toBeInTheDocument();
  });

  it("ofrece reintentar el último dictado fallido", async () => {
    setCommands({ history_list: [], dictation_retry_available: true });
    render(<History modes={fx.modes} />);
    fireEvent.click(await screen.findByRole("button", { name: "Reintentar" }));
    await screen.findByText("Reintentando…");
    expect(called("dictation_retry")).toHaveLength(1);
    act(() => fire("dictation-retry-available", true));
    expect(screen.getByRole("alert")).toHaveTextContent("El audio sigue en memoria");
  });
});
