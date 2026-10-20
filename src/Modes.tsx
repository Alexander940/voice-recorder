import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

export default function Modes() {
  const [state, setState] = useState<ModesState | null>(null);
  const [editing, setEditing] = useState<Mode | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ModesState>("modes_list")
      .then(setState)
      .catch((e) => setError(String(e)));
  }, []);

  async function call(cmd: string, args: Record<string, unknown>) {
    try {
      setState(await invoke<ModesState>(cmd, args));
      setError(null);
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }

  async function save() {
    if (editing && (await call("modes_save", { mode: editing }))) setEditing(null);
  }

  if (!state) return <p>{error ?? "Cargando modos…"}</p>;

  return (
    <section>
      <h3>Modos</h3>
      {error && <p role="alert">{error}</p>}
      <ul aria-label="Modos">
        {state.modes.map((m) => (
          <li key={m.id}>
            <label>
              <input
                type="radio"
                name="active-mode"
                checked={state.active === m.id}
                onChange={() => call("modes_set_active", { id: m.id })}
              />
              {m.name}
            </label>
            <button onClick={() => setEditing({ ...m })}>Editar</button>
            <button onClick={() => call("modes_duplicate", { id: m.id })}>Duplicar</button>
            {!m.builtin && (
              <button onClick={() => call("modes_delete", { id: m.id })}>Borrar</button>
            )}
          </li>
        ))}
      </ul>
      <button onClick={() => setEditing({ ...EMPTY })}>Nuevo modo</button>

      {editing && (
        <form
          aria-label="Editar modo"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <label>
            Nombre
            <input
              value={editing.name}
              onChange={(e) => setEditing({ ...editing, name: e.target.value })}
            />
          </label>
          <label>
            Instrucciones
            <textarea
              value={editing.prompt}
              disabled={!editing.use_llm}
              onChange={(e) => setEditing({ ...editing, prompt: e.target.value })}
            />
          </label>
          <label>
            Idioma de salida
            <input
              placeholder="El del dictado"
              value={editing.output_language ?? ""}
              disabled={!editing.use_llm}
              onChange={(e) =>
                setEditing({
                  ...editing,
                  output_language: e.target.value.trim() ? e.target.value : null,
                })
              }
            />
          </label>
          <label>
            <input
              type="checkbox"
              checked={editing.use_llm}
              onChange={(e) => setEditing({ ...editing, use_llm: e.target.checked })}
            />
            Procesar con Claude
          </label>
          <button type="submit">Guardar</button>
          <button type="button" onClick={() => setEditing(null)}>
            Cancelar
          </button>
        </form>
      )}
    </section>
  );
}
