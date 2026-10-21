// Backend de Tauri simulado para los tests de componentes.
import { vi } from "vitest";

type Handler = (e: { payload: unknown }) => void;
type Command = unknown | ((args: Record<string, unknown>) => unknown);

export const handlers: Record<string, Handler> = {};
export const calls: { cmd: string; args?: Record<string, unknown> }[] = [];
let table: Record<string, Command> = {};

/** Rechazo de un comando, como lo devuelve Tauri (un string). */
export class Reject {
  constructor(public message: string) {}
}

export function setCommands(t: Record<string, Command>) {
  table = t;
  calls.length = 0;
  for (const k of Object.keys(handlers)) delete handlers[k];
}

export const invoke = vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
  calls.push({ cmd, args });
  const value = table[cmd];
  const out = typeof value === "function" ? value(args ?? {}) : value;
  if (out instanceof Reject) throw out.message;
  return out === undefined ? null : out;
});

export const listen = vi.fn((name: string, handler: Handler) => {
  handlers[name] = handler;
  return Promise.resolve(() => delete handlers[name]);
});

export function fire(name: string, payload: unknown) {
  handlers[name]?.({ payload });
}

export const called = (cmd: string) => calls.filter((c) => c.cmd === cmd);
