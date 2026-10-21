import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PlusIcon } from "./icons";
import { modeShortcut } from "./platform";

export interface Mode {
  id: string;
  name: string;
  prompt: string;
  output_language: string | null;
  use_llm: boolean;
  builtin: boolean;
}

export interface ModesState {
  modes: Mode[];
  active: string;
}

const EMPTY: Mode = {
  id: "",
  name: "",
  prompt: "",
  output_language: null,
  use_llm: true,
  builtin: false,
};

interface Props {
  state: ModesState | null;
  onChange: (state: ModesState) => void;
}

function tag(m: Mode, active: boolean): string | null {
  if (active) return "Activo";
  if (!m.use_llm) return "sin IA";
  if (!m.builtin) return "tuyo";
  return null;
}

export default function Modes({ state, onChange }: Props) {
  // Modo en edición: por defecto, el activo. `null` + `creating` = modo nuevo.
  const [editing, setEditing] = useState<Mode | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function call(cmd: string, args: Record<string, unknown>) {
    try {
      const next = await invoke<ModesState>(cmd, args);
      onChange(next);
      setError(null);
      return next;
    } catch (e) {
      setError(String(e));
      return null;
    }
  }

  if (!state) return <main className="page">{error ?? "Cargando modos…"}</main>;

  const active = state.modes.find((m) => m.id === state.active) ?? state.modes[0];
  const draft = editing ?? active;
  const stored = state.modes.find((m) => m.id === draft?.id);
  const kicker = !draft?.id
    ? "Nuevo modo"
    : stored?.builtin
      ? "Modo predefinido"
      : "Modo personalizado";

  async function activate(m: Mode) {
    setEditing(null);
    await call("modes_set_active", { id: m.id });
  }

  async function save() {
    if (!draft) return;
    const next = await call("modes_save", { mode: draft });
    if (next) {
      // Tras guardar se sigue editando el modo guardado (el nuevo recibe id en el backend).
      const saved =
        next.modes.find((m) => m.id === draft.id) ??
        next.modes.find((m) => m.name === draft.name.trim());
      setEditing(saved ?? null);
    }
  }

  async function duplicate() {
    if (!draft?.id) return;
    const next = await call("modes_duplicate", { id: draft.id });
    if (next) setEditing(next.modes[next.modes.length - 1]);
  }

  async function remove() {
    if (!draft?.id) return;
    if (await call("modes_delete", { id: draft.id })) setEditing(null);
  }

  const set = (patch: Partial<Mode>) => draft && setEditing({ ...draft, ...patch });

  return (
    <main className="modes">
      <div className="modes-list">
        <div className="modes-head">
          <h1 className="page-title">Modos</h1>
          <button type="button" className="btn btn-new" onClick={() => setEditing({ ...EMPTY })}>
            <PlusIcon size={14} />
            Nuevo
          </button>
        </div>
        <p className="modes-help">
          El modo activo decide cómo Claude da formato a cada dictado. Cámbialo con{" "}
          <span className="mono">{modeShortcut()}</span>.
        </p>
        {error && (
          <p role="alert" className="modes-help error-text">
            {error}
          </p>
        )}
        <fieldset className="modes-fieldset">
          <legend className="sr-only">Modo activo</legend>
          {state.modes.map((m) => {
            const isActive = m.id === state.active;
            const t = tag(m, isActive);
            return (
              <label key={m.id} className={isActive ? "mode-row active" : "mode-row"}>
                <input
                  type="radio"
                  name="active-mode"
                  className="radio"
                  checked={isActive}
                  onChange={() => activate(m)}
                />
                {m.name}
                {t && <span className="mode-tag">{t}</span>}
              </label>
            );
          })}
        </fieldset>
      </div>
      {draft && (
        <form
          aria-label="Editar modo"
          className="mode-editor"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <div className="editor-head">
            <span className="kicker-sm">{kicker}</span>
            <div className="editor-tools">
              {draft.id && (
                <button type="button" className="btn btn-xs pad-12" onClick={duplicate}>
                  Duplicar
                </button>
              )}
              {draft.id && stored && !stored.builtin && (
                <button type="button" className="btn btn-xs pad-12" onClick={remove}>
                  Borrar
                </button>
              )}
            </div>
          </div>
          <label className="field-label">
            Nombre
            <input
              className="input"
              value={draft.name}
              onChange={(e) => set({ name: e.target.value })}
            />
          </label>
          <label className="field-label">
            Instrucciones para Claude
            <textarea
              className="textarea"
              rows={6}
              value={draft.prompt}
              disabled={!draft.use_llm}
              onChange={(e) => set({ prompt: e.target.value })}
            />
          </label>
          <div className="two-cols">
            <label className="field-label">
              Idioma de salida
              <input
                className="input plain"
                placeholder="El del dictado"
                value={draft.output_language ?? ""}
                disabled={!draft.use_llm}
                onChange={(e) =>
                  set({ output_language: e.target.value.trim() ? e.target.value : null })
                }
              />
            </label>
            <label className="check-line">
              <input
                type="checkbox"
                className="checkbox"
                checked={draft.use_llm}
                onChange={(e) => set({ use_llm: e.target.checked })}
              />
              Procesar con Claude
            </label>
          </div>
          <div className="grow" />
          <div className="form-foot">
            <button type="button" className="btn btn-secondary" onClick={() => setEditing(null)}>
              Cancelar
            </button>
            <button type="submit" className="btn btn-primary">
              Guardar
            </button>
          </div>
        </form>
      )}
    </main>
  );
}
