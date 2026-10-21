import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Updates, { lastChecked, noteLines, progressText } from "./Updates";
import * as fx from "./harness/fixtures";
import { called, fire, Reject, setCommands } from "./test-tauri";

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

  it("texto del progreso de descarga", () => {
    expect(progressText(null)).toBe("Descargando…");
    expect(progressText({ downloaded: 3, total: 4 })).toBe("Descargando… 75 %");
    expect(progressText({ downloaded: 2_097_152, total: null })).toBe("Descargando… 2,0 MB");
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

  it("nunca instala sin confirmación y no usa window.confirm", async () => {
    // En el WebView de macOS window.confirm no se muestra y devuelve false.
    const confirm = vi.spyOn(window, "confirm");
    render(<Updates />);
    fireEvent.click(await screen.findByRole("button", { name: "Instalar y reiniciar" }));
    const dialog = screen.getByRole("alertdialog", { name: "Confirmar la instalación" });
    expect(dialog).toHaveTextContent("Se descargará la versión 0.2.0");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(called("update_install")).toHaveLength(0);
    expect(confirm).not.toHaveBeenCalled();
  });

  it("instala tras confirmar y muestra el progreso", async () => {
    setCommands({
      app_version: "0.1.0",
      update_status: fx.update,
      update_install: () => new Promise(() => {}),
    });
    render(<Updates />);
    fireEvent.click(await screen.findByRole("button", { name: "Instalar y reiniciar" }));
    fireEvent.click(screen.getByRole("button", { name: "Sí, instalar" }));
    await vi.waitFor(() => expect(called("update_install")).toHaveLength(1));
    expect(screen.getByRole("button", { name: "Descargando…" })).toBeDisabled();
    act(() => fire("update-progress", { downloaded: 512, total: 1024 }));
    expect(screen.getByRole("button", { name: "Descargando… 50 %" })).toBeInTheDocument();
  });

  it("muestra el error si la instalación falla", async () => {
    setCommands({
      app_version: "0.1.0",
      update_status: fx.update,
      update_install: () => new Reject("firma inválida"),
    });
    render(<Updates />);
    fireEvent.click(await screen.findByRole("button", { name: "Instalar y reiniciar" }));
    fireEvent.click(screen.getByRole("button", { name: "Sí, instalar" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("firma inválida");
    expect(screen.getByRole("button", { name: "Instalar y reiniciar" })).toBeEnabled();
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
