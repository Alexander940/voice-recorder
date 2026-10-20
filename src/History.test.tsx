import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import History, { type HistoryEntry } from "./History";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

type Handler = (e: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    handlers[name] = handler;
    return Promise.resolve(() => delete handlers[name]);
  }),
}));

let retryAvailable = false;

const entry = (id: number, text: string, mode = "clean"): HistoryEntry => ({
  id,
  created_at: 1_760_000_000_000,
  raw_text: `eh ${text}`,
  final_text: text,
  mode,
  duration_ms: 2500,
  stt_latency_ms: 900,
  llm_latency_ms: 700,
});

let db: HistoryEntry[] = [];

beforeEach(() => {
  retryAvailable = false;
  db = [entry(2, "Comprar pan"), entry(1, "Reunión con ventas", "literal")];
  invoke.mockReset();
  invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
    if (cmd === "history_list") {
      const q = String(args.query).toLowerCase();
      return Promise.resolve(db.filter((e) => e.final_text.toLowerCase().includes(q)));
    }
    if (cmd === "dictation_retry_available") return Promise.resolve(retryAvailable);
    if (cmd === "history_delete") {
      db = db.filter((e) => e.id !== args.id);
      return Promise.resolve(null);
    }
    return Promise.resolve(null);
  });
});

describe("History", () => {
  it("lista los dictados con duración y tipo", async () => {
    render(<History />);
    const items = await screen.findAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("Comprar pan");
    expect(items[0]).toHaveTextContent("2.5 s");
    expect(items[1]).toHaveTextContent("literal");
  });

  it("busca enviando la consulta al backend", async () => {
    render(<History />);
    await screen.findAllByRole("listitem");
    fireEvent.change(screen.getByLabelText("Buscar en el historial"), {
      target: { value: "ventas" },
    });
    await act(async () => {});
    expect(invoke).toHaveBeenLastCalledWith("history_list", { query: "ventas" });
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
  });

  it("sin resultados muestra un mensaje", async () => {
    db = [];
    render(<History />);
    expect(await screen.findByText("Todavía no hay dictados.")).toBeInTheDocument();
  });

  it("copiar y pegar de nuevo llaman a history_deliver", async () => {
    render(<History />);
    const [first] = await screen.findAllByRole("listitem");
    await act(async () => fireEvent.click(within(first).getByText("Copiar")));
    expect(invoke).toHaveBeenCalledWith("history_deliver", { id: 2, paste: false });
    expect(screen.getByRole("status")).toHaveTextContent("Copiado");
    await act(async () => fireEvent.click(within(first).getByText("Pegar de nuevo")));
    expect(invoke).toHaveBeenCalledWith("history_deliver", { id: 2, paste: true });
  });

  it("borrar elimina la entrada y recarga", async () => {
    render(<History />);
    const [first] = await screen.findAllByRole("listitem");
    await act(async () => fireEvent.click(within(first).getByText("Borrar")));
    expect(invoke).toHaveBeenCalledWith("history_delete", { id: 2 });
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
  });

  it("muestra el error del backend", async () => {
    render(<History />);
    const [first] = await screen.findAllByRole("listitem");
    invoke.mockRejectedValueOnce("No se pudo pegar");
    await act(async () => fireEvent.click(within(first).getByText("Pegar de nuevo")));
    expect(screen.getByRole("status")).toHaveTextContent("No se pudo pegar");
  });

  it("muestra el reintento cuando un dictado falló y lo lanza", async () => {
    retryAvailable = true;
    render(<History />);
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("no se pudo procesar");
    await act(async () => fireEvent.click(within(alert).getByText("Reintentar")));
    expect(invoke).toHaveBeenCalledWith("dictation_retry");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("el evento del backend muestra u oculta el reintento", async () => {
    render(<History />);
    await screen.findAllByRole("listitem");
    act(() => handlers["dictation-retry-available"]({ payload: true }));
    expect(screen.getByRole("alert")).toBeInTheDocument();
    act(() => handlers["dictation-retry-available"]({ payload: false }));
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
