import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue("0.1.0"),
}));

describe("App", () => {
  it("muestra el título y la versión del backend", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Voice Recorder" })).toBeInTheDocument();
    expect(await screen.findByTestId("version")).toHaveTextContent("v0.1.0");
  });
});
