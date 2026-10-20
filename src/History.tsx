import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface HistoryEntry {
  id: number;
  created_at: number;
  raw_text: string;
  final_text: string;
  mode: string;
  duration_ms: number;
  stt_latency_ms: number;
  llm_latency_ms: number | null;
}

export function formatDate(ms: number): string {
  return new Date(ms).toLocaleString("es", { dateStyle: "short", timeStyle: "short" });
}

export default function History() {
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  // Se incrementa para recargar tras borrar.
  const [revision, setRevision] = useState(0);
  const reload = useCallback(() => setRevision((r) => r + 1), []);

  useEffect(() => {
    let current = true;
    invoke<HistoryEntry[]>("history_list", { query })
      .then((list) => current && setEntries(list))
      .catch(() => current && setEntries([]));
    return () => {
      current = false;
    };
  }, [query, revision]);

  async function run(action: () => Promise<unknown>, ok: string) {
    try {
      await action();
      setMessage(ok);
    } catch (e) {
      setMessage(String(e));
    }
  }

  return (
    <section>
      <input
        type="search"
        aria-label="Buscar en el historial"
        placeholder="Buscar…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      {message && <p role="status">{message}</p>}
      {entries.length === 0 ? (
        <p>{query ? "Sin resultados." : "Todavía no hay dictados."}</p>
      ) : (
        <ul aria-label="Dictados">
          {entries.map((e) => (
            <li key={e.id}>
              <p>{e.final_text}</p>
              <small>
                {formatDate(e.created_at)} · {(e.duration_ms / 1000).toFixed(1)} s ·{" "}
                {e.mode === "literal" ? "literal" : `modo ${e.mode}`}
              </small>
              <div>
                <button
                  onClick={() =>
                    run(() => invoke("history_deliver", { id: e.id, paste: false }), "Copiado")
                  }
                >
                  Copiar
                </button>
                <button
                  onClick={() =>
                    run(() => invoke("history_deliver", { id: e.id, paste: true }), "Pegado")
                  }
                >
                  Pegar de nuevo
                </button>
                <button
                  onClick={() =>
                    run(async () => {
                      await invoke("history_delete", { id: e.id });
                      reload();
                    }, "Borrado")
                  }
                >
                  Borrar
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
