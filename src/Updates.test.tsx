import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Updates, { type UpdateStatus } from "./Updates";

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

let status: UpdateStatus;

beforeEach(() => {
  status = { configured: true, enabled: true, available: null };
  invoke.mockReset();
  invoke.mockImplementation((cmd: string) =>
    Promise.resolve(cmd === "update_status" ? status : null),
  );
});

async function renderUpdates(onToggle = vi.fn()) {
  render(<Updates enabled onToggle={onToggle} />);
  await act(async () => {});
  return onToggle;
}

describe("Updates", () => {
  it("el interruptor desactiva las comprobaciones", async () => {
    const onToggle = await renderUpdates();
    fireEvent.click(screen.getByLabelText("Buscar actualizaciones al iniciar y una vez al día"));
    expect(onToggle).toHaveBeenCalledWith(false);
  });

  it("avisa si no está configurado en esta compilación", async () => {
    status = { configured: false, enabled: true, available: null };
    await renderUpdates();
    expect(screen.getByText(/aún no están configuradas/)).toBeInTheDocument();
  });

  it("muestra la versión disponible cuando llega el evento", async () => {
    await renderUpdates();
    expect(screen.queryByRole("status")).toBeNull();
    act(() => handlers["update-available"]({ payload: { version: "0.2.0", notes: "Mejoras" } }));
    expect(screen.getByRole("status")).toHaveTextContent("0.2.0");
    expect(screen.getByRole("status")).toHaveTextContent("Mejoras");
  });

  it("no instala sin confirmación", async () => {
    status = { ...status, available: { version: "0.2.0", notes: null } };
    await renderUpdates();
    vi.spyOn(window, "confirm").mockReturnValue(false);
    await act(async () => fireEvent.click(screen.getByText("Instalar y reiniciar")));
    expect(invoke).not.toHaveBeenCalledWith("update_install");
  });

  it("instala tras confirmar y muestra el error si falla", async () => {
    status = { ...status, available: { version: "0.2.0", notes: null } };
    await renderUpdates();
    vi.spyOn(window, "confirm").mockReturnValue(true);
    invoke.mockRejectedValueOnce("La firma de la actualización no es válida; no se instalará");
    await act(async () => fireEvent.click(screen.getByText("Instalar y reiniciar")));
    expect(invoke).toHaveBeenCalledWith("update_install");
    expect(screen.getByRole("alert")).toHaveTextContent("firma");
  });
});
