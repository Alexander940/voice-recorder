import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Accounts from "./Accounts";
import { called, Reject, setCommands } from "./test-tauri";

vi.mock("@tauri-apps/api/core", () => import("./test-tauri"));

describe("Accounts", () => {
  beforeEach(() =>
    setCommands({
      get_key_status: { groq: true, anthropic: false },
      get_key_hints: { groq: "gsk_••••7f2a", anthropic: null },
      save_api_key: { groq: true, anthropic: true },
      delete_api_key: { groq: false, anthropic: false },
    }),
  );

  it("una key configurada muestra solo su pista", async () => {
    render(<Accounts />);
    const groq = (await screen.findByText("Groq")).closest("li")!;
    expect(within(groq).getByText("Configurada")).toBeInTheDocument();
    expect(within(groq).getByText("gsk_••••7f2a")).toBeInTheDocument();
    expect(within(groq).getByRole("button", { name: "Cambiar" })).toBeInTheDocument();
  });

  it("una key sin configurar muestra el formulario y la guarda", async () => {
    render(<Accounts />);
    const anthropic = (await screen.findByText("Anthropic")).closest("li")!;
    expect(within(anthropic).getByText("Sin configurar")).toBeInTheDocument();
    const save = within(anthropic).getByRole("button", { name: "Validar y guardar" });
    expect(save).toBeDisabled();
    fireEvent.change(within(anthropic).getByLabelText("API key de Anthropic"), {
      target: { value: "sk-ant-secreta" },
    });
    fireEvent.click(save);
    await vi.waitFor(() => expect(called("save_api_key")).toHaveLength(1));
    expect(called("save_api_key")[0].args).toEqual({
      provider: "anthropic",
      key: "sk-ant-secreta",
    });
    // La key no se queda en el campo.
    await vi.waitFor(() => expect(called("get_key_status").length).toBeGreaterThan(1));
  });

  it("explica una key con formato incorrecto", async () => {
    setCommands({
      get_key_status: { groq: false, anthropic: false },
      get_key_hints: { groq: null, anthropic: null },
      save_api_key: () => new Reject("No parece una key de Groq: debe empezar por «gsk_»"),
    });
    render(<Accounts />);
    const groq = (await screen.findByText("Groq")).closest("li")!;
    fireEvent.change(within(groq).getByLabelText("API key de Groq"), {
      target: { value: "xai-123" },
    });
    fireEvent.click(within(groq).getByRole("button", { name: "Validar y guardar" }));
    const alert = await within(groq).findByRole("alert");
    expect(alert).toHaveTextContent("Esta key no parece de Groq: debe empezar por gsk_.");
    expect(alert).toHaveTextContent("Las keys de xAI (Grok) no sirven.");
  });

  it("borra una key", async () => {
    render(<Accounts />);
    const groq = (await screen.findByText("Groq")).closest("li")!;
    fireEvent.click(within(groq).getByRole("button", { name: "Borrar" }));
    await vi.waitFor(() => expect(called("delete_api_key")[0].args).toEqual({ provider: "groq" }));
  });
});
