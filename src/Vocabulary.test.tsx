import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Vocabulary, { addTerm } from "./Vocabulary";
import * as fx from "./harness/fixtures";
import { called, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));

describe("addTerm", () => {
  it("ignora vacíos y repetidos sin distinguir mayúsculas", () => {
    expect(addTerm(["Groq"], "  ")).toEqual(["Groq"]);
    expect(addTerm(["Groq"], "groq")).toEqual(["Groq"]);
    expect(addTerm(["Groq"], " Tauri ")).toEqual(["Groq", "Tauri"]);
  });
});

describe("Vocabulary", () => {
  beforeEach(() =>
    setCommands({
      vocabulary_get: fx.vocabulary,
      vocabulary_save: (a: Record<string, unknown>) => a.vocabulary,
    }),
  );

  it("muestra los términos como chips y los reemplazos", async () => {
    render(<Vocabulary />);
    expect(await screen.findByText("Kubernetes")).toBeInTheDocument();
    expect(screen.getByLabelText("Si oyes 1")).toHaveValue("cloud");
    expect(screen.getByLabelText("Escribe 2")).toHaveValue("@empresa.com");
  });

  it("añade con Enter y quita con la ×", async () => {
    render(<Vocabulary />);
    await screen.findByText("Kubernetes");
    const input = screen.getByLabelText("Añadir término");
    fireEvent.change(input, { target: { value: "Vite" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByText("Vite")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Quitar Kubernetes" }));
    expect(screen.queryByText("Kubernetes")).toBeNull();
  });

  it("añade y quita reemplazos y guarda todo", async () => {
    render(<Vocabulary />);
    await screen.findByLabelText("Si oyes 1");
    fireEvent.click(screen.getByRole("button", { name: "Quitar reemplazo 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Añadir reemplazo" }));
    fireEvent.change(screen.getByLabelText("Si oyes 2"), { target: { value: "jason" } });
    fireEvent.change(screen.getByLabelText("Escribe 2"), { target: { value: "JSON" } });
    // Un término a medio escribir también se guarda.
    fireEvent.change(screen.getByLabelText("Añadir término"), { target: { value: "Rust" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar vocabulario" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Guardado");
    expect(called("vocabulary_save")[0].args).toEqual({
      vocabulary: {
        terms: ["Kubernetes", "Anthropic", "Tauri", "Groq", "Rust"],
        replacements: [
          { from: "arroba empresa", to: "@empresa.com" },
          { from: "jason", to: "JSON" },
        ],
      },
    });
  });
});
