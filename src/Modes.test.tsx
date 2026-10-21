import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Modes, { type ModesState } from "./Modes";
import * as fx from "./harness/fixtures";
import { called, Reject, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));

const state: ModesState = { ...fx.modes, active: "email" };

describe("Modes", () => {
  beforeEach(() =>
    setCommands({
      modes_set_active: (a: Record<string, unknown>) => ({ ...state, active: a.id }),
      modes_save: () => state,
      modes_delete: () => state,
      modes_duplicate: () => state,
    }),
  );

  it("lista los modos con su etiqueta y edita el activo", () => {
    render(<Modes state={state} onChange={() => {}} />);
    expect(screen.getByRole("radio", { name: /Email/ })).toBeChecked();
    expect(screen.getByText("Activo")).toBeInTheDocument();
    expect(screen.getByText("sin IA")).toBeInTheDocument();
    expect(screen.getByText("tuyo")).toBeInTheDocument();
    expect(screen.getByText("Modo predefinido")).toBeInTheDocument();
    expect(screen.getByLabelText("Nombre")).toHaveValue("Email");
    expect(screen.getByLabelText("Instrucciones para Claude")).toHaveValue(
      fx.modes.modes[2].prompt,
    );
    // Los predefinidos no se pueden borrar.
    expect(screen.queryByRole("button", { name: "Borrar" })).toBeNull();
  });

  it("elegir un modo lo activa", async () => {
    const onChange = vi.fn();
    render(<Modes state={state} onChange={onChange} />);
    fireEvent.click(screen.getByRole("radio", { name: /Mensaje/ }));
    await vi.waitFor(() => expect(onChange).toHaveBeenCalled());
    expect(called("modes_set_active")[0].args).toEqual({ id: "message" });
  });

  it("crea un modo nuevo y lo guarda", async () => {
    render(<Modes state={state} onChange={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Nuevo" }));
    expect(screen.getByText("Nuevo modo")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Nombre"), { target: { value: "Notas" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await vi.waitFor(() => expect(called("modes_save")).toHaveLength(1));
    expect(called("modes_save")[0].args).toMatchObject({ mode: { id: "", name: "Notas" } });
  });

  it("un modo propio se puede borrar", async () => {
    render(<Modes state={{ ...state, active: "tweets" }} onChange={() => {}} />);
    expect(screen.getByText("Modo personalizado")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Borrar" }));
    await vi.waitFor(() => expect(called("modes_delete")[0].args).toEqual({ id: "tweets" }));
  });

  it("sin Claude se desactivan las instrucciones", () => {
    render(<Modes state={state} onChange={() => {}} />);
    fireEvent.click(screen.getByLabelText("Procesar con Claude"));
    expect(screen.getByLabelText("Instrucciones para Claude")).toBeDisabled();
  });

  it("muestra los errores del backend", async () => {
    setCommands({ modes_save: () => new Reject("El nombre no puede estar vacío") });
    render(<Modes state={state} onChange={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("El nombre no puede estar vacío");
  });
});
