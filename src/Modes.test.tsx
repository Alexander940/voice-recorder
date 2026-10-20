import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Modes, { type Mode, type ModesState } from "./Modes";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

const mode = (id: string, name: string, builtin = true, use_llm = true): Mode => ({
  id,
  name,
  prompt: "",
  output_language: null,
  use_llm,
  builtin,
});

let state: ModesState;

beforeEach(() => {
  state = {
    modes: [
      mode("literal", "Literal", true, false),
      mode("clean", "Limpio"),
      mode("custom-1", "Tuits", false),
    ],
    active: "clean",
  };
  invoke.mockReset();
  invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
    if (cmd === "modes_set_active") state = { ...state, active: String(args.id) };
    if (cmd === "modes_delete")
      state = { ...state, modes: state.modes.filter((m) => m.id !== args.id) };
    if (cmd === "modes_save") {
      const m = args.mode as Mode;
      state = { ...state, modes: [...state.modes, { ...m, id: "custom-2" }] };
    }
    if (cmd === "modes_duplicate")
      state = { ...state, modes: [...state.modes, mode("custom-3", "Limpio (copia)", false)] };
    return Promise.resolve(state);
  });
});

const items = () => screen.getAllByRole("listitem");

describe("Modes", () => {
  it("lista los modos y marca el activo", async () => {
    render(<Modes />);
    await screen.findByText("Tuits");
    expect(items()).toHaveLength(3);
    expect(within(items()[1]).getByRole("radio")).toBeChecked();
  });

  it("los predefinidos no se pueden borrar; los propios sí", async () => {
    render(<Modes />);
    await screen.findByText("Tuits");
    expect(within(items()[0]).queryByText("Borrar")).toBeNull();
    await act(async () => fireEvent.click(within(items()[2]).getByText("Borrar")));
    expect(invoke).toHaveBeenCalledWith("modes_delete", { id: "custom-1" });
    expect(items()).toHaveLength(2);
  });

  it("cambia el modo activo", async () => {
    render(<Modes />);
    await screen.findByText("Tuits");
    await act(async () => fireEvent.click(within(items()[0]).getByRole("radio")));
    expect(invoke).toHaveBeenCalledWith("modes_set_active", { id: "literal" });
    expect(within(items()[0]).getByRole("radio")).toBeChecked();
  });

  it("crea un modo nuevo con nombre, instrucciones e idioma", async () => {
    render(<Modes />);
    await screen.findByText("Tuits");
    fireEvent.click(screen.getByText("Nuevo modo"));
    const form = screen.getByRole("form", { name: "Editar modo" });
    fireEvent.change(within(form).getByLabelText("Nombre"), { target: { value: "Notas" } });
    fireEvent.change(within(form).getByLabelText("Instrucciones"), {
      target: { value: "Viñetas." },
    });
    fireEvent.change(within(form).getByLabelText("Idioma de salida"), {
      target: { value: "francés" },
    });
    await act(async () => fireEvent.click(within(form).getByText("Guardar")));
    expect(invoke).toHaveBeenCalledWith("modes_save", {
      mode: expect.objectContaining({
        id: "",
        name: "Notas",
        prompt: "Viñetas.",
        output_language: "francés",
      }),
    });
    expect(screen.queryByRole("form")).toBeNull();
    expect(screen.getByText("Notas")).toBeInTheDocument();
  });

  it("sin Claude se desactivan las instrucciones", async () => {
    render(<Modes />);
    await screen.findByText("Tuits");
    fireEvent.click(within(items()[0]).getByText("Editar"));
    expect(screen.getByLabelText("Instrucciones")).toBeDisabled();
  });

  it("duplica y muestra el error del backend", async () => {
    render(<Modes />);
    await screen.findByText("Tuits");
    await act(async () => fireEvent.click(within(items()[1]).getByText("Duplicar")));
    expect(screen.getByText("Limpio (copia)")).toBeInTheDocument();
    invoke.mockRejectedValueOnce("Los modos predefinidos no se pueden borrar");
    await act(async () => fireEvent.click(within(items()[3]).getByText("Borrar")));
    expect(screen.getByRole("alert")).toHaveTextContent("no se pueden borrar");
  });
});
