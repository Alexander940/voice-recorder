import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Updates, { lastChecked, noteLines } from "./Updates";
import * as fx from "./harness/fixtures";
import { called, fire, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));
vi.mock("@tauri-apps/api/event", () => import("./test-tauri"));

describe("helpers", () => {
  it("formatea la última comprobación", () => {
    const now = new Date(2026, 9, 21, 12).getTime();
    expect(lastChecked(new Date(2026, 9, 21, 9).getTime(), now)).toBe(
      "Última comprobación: hoy a las 09:00",
    );
    expect(lastChecked(new Date(2026, 9, 20, 18, 30).getTime(), now)).toMatch(/ayer a las 18:30/);
    expect(lastChecked(null, now)).toBe("Todavía no se ha comprobado");
  });

  it("separa las notas en líneas sin viñetas", () => {
    expect(noteLines("- Uno\n\n* Dos\nTres")).toEqual(["Uno", "Dos", "Tres"]);
    expect(noteLines(null)).toEqual([]);
  });
});

describe("Updates", () => {
  beforeEach(() =>
    setCommands({
      app_version: "0.1.0",
      update_status: fx.update,
      update_check_now: { ...fx.update, available: null },
      settings_get: fx.settings,
      settings_update: (a: Record<string, unknown>) => a.settings,
    }),
  );
  afterEach(() => vi.restoreAllMocks());

  it("muestra la versión, la nueva versión y sus notas", async () => {
    render(<Updates />);
    expect(await screen.findByText("Voice Recorder 0.1.0")).toBeInTheDocument();
    expect(screen.getByText("0.2.0")).toBeInTheDocument();
    expect(screen.getByText("[NOTA DE LA VERSIÓN 1]")).toBeInTheDocument();
    expect(screen.getByText(/Última comprobación: hoy a las 09:00/)).toBeInTheDocument();
  });

  it("nunca instala sin confirmación", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<Updates />);
    fireEvent.click(await screen.findByRole("button", { name: "Instalar y reiniciar" }));
    expect(confirm).toHaveBeenCalled();
    expect(called("update_install")).toHaveLength(0);
  });

  it("instala tras confirmar", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<Updates />);
    fireEvent.click(await screen.findByRole("button", { name: "Instalar y reiniciar" }));
    await vi.waitFor(() => expect(called("update_install")).toHaveLength(1));
  });

  it("busca ahora y desactiva la comprobación automática", async () => {
    render(<Updates />);
    fireEvent.click(await screen.findByRole("button", { name: "Buscar ahora" }));
    await vi.waitFor(() => expect(screen.queryByText("0.2.0")).toBeNull());
    fireEvent.click(screen.getByRole("switch", { name: /Buscar actualizaciones/ }));
    await vi.waitFor(() => expect(called("settings_update")).toHaveLength(1));
    expect(called("settings_update")[0].args).toMatchObject({
      settings: { updates: { enabled: false } },
    });
  });

  it("avisa por evento de una versión nueva", async () => {
    setCommands({ app_version: "0.1.0", update_status: { ...fx.update, available: null } });
    render(<Updates />);
    await screen.findByText("Voice Recorder 0.1.0");
    act(() => fire("update-available", { version: "0.3.0", notes: null }));
    expect(screen.getByText("0.3.0")).toBeInTheDocument();
  });

  it("sin configurar lo explica y no permite buscar", async () => {
    setCommands({ app_version: "0.1.0", update_status: { ...fx.update, configured: false } });
    render(<Updates />);
    expect(await screen.findByText(/aún no están configuradas/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Buscar ahora" })).toBeDisabled();
  });
});
