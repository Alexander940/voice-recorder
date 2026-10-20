import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Vocabulary from "./Vocabulary";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd: string, args: Record<string, unknown>) => {
    if (cmd === "vocabulary_get") {
      return Promise.resolve({
        terms: ["Kubernetes", "Tauri"],
        replacements: [{ from: "cubernetes", to: "Kubernetes" }],
      });
    }
    // El backend normaliza: quita líneas vacías.
    const v = args.vocabulary as { terms: string[]; replacements: unknown[] };
    return Promise.resolve({ ...v, terms: v.terms.filter((t) => t.trim()) });
  });
});

describe("Vocabulary", () => {
  it("carga términos y reemplazos", async () => {
    render(<Vocabulary />);
    expect(await screen.findByLabelText("Términos (uno por línea)")).toHaveValue(
      "Kubernetes\nTauri",
    );
    expect(screen.getByLabelText("Si oyes 1")).toHaveValue("cubernetes");
    expect(screen.getByLabelText("Escribe 1")).toHaveValue("Kubernetes");
  });

  it("guarda términos por línea y reemplazos añadidos", async () => {
    render(<Vocabulary />);
    const textarea = await screen.findByLabelText("Términos (uno por línea)");
    await act(async () => {});
    fireEvent.change(textarea, { target: { value: "Rust\n\nAnthropic" } });
    fireEvent.click(screen.getByText("Añadir reemplazo"));
    fireEvent.change(screen.getByLabelText("Si oyes 2"), { target: { value: "ban colombia" } });
    fireEvent.change(screen.getByLabelText("Escribe 2"), { target: { value: "Bancolombia" } });
    await act(async () => fireEvent.click(screen.getByText("Guardar vocabulario")));
    expect(invoke).toHaveBeenCalledWith("vocabulary_save", {
      vocabulary: {
        terms: ["Rust", "", "Anthropic"],
        replacements: [
          { from: "cubernetes", to: "Kubernetes" },
          { from: "ban colombia", to: "Bancolombia" },
        ],
      },
    });
    expect(screen.getByRole("status")).toHaveTextContent("Vocabulario guardado");
    expect(textarea).toHaveValue("Rust\nAnthropic");
  });

  it("quita un reemplazo", async () => {
    render(<Vocabulary />);
    await screen.findByLabelText("Si oyes 1");
    fireEvent.click(screen.getByText("Quitar"));
    expect(screen.queryByLabelText("Si oyes 1")).toBeNull();
  });
});
