import { Fragment, useCallback, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SearchIcon, TrashIcon, WarnIcon } from "./icons";
import type { ModesState } from "./Modes";
import { seconds } from "./platform";

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

const DAY = 24 * 60 * 60 * 1000;

function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** "Hoy", "Ayer" o la fecha. */
export function dayLabel(ms: number, now = Date.now()): string {
  const diff = Math.round((startOfDay(now) - startOfDay(ms)) / DAY);
  if (diff === 0) return "Hoy";
  if (diff === 1) return "Ayer";
  return new Date(ms).toLocaleDateString("es", {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
}

export function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
}

/** Agrupa por día manteniendo el orden (del más reciente al más antiguo). */
export function groupByDay(entries: HistoryEntry[], now = Date.now()) {
  const groups: { label: string; entries: HistoryEntry[] }[] = [];
  for (const e of entries) {
    const label = dayLabel(e.created_at, now);
    const last = groups[groups.length - 1];
    if (last?.label === label) last.entries.push(e);
    else groups.push({ label, entries: [e] });
  }
  return groups;
}

/** El texto entre `comillas invertidas` se muestra como código. */
export function withCode(text: string): ReactNode {
  return text.split(/(`[^`]+`)/).map((part, i) =>
    part.startsWith("`") && part.endsWith("`") && part.length > 2 ? (
      <span key={i} className="code">
        {part.slice(1, -1)}
      </span>
    ) : (
      <Fragment key={i}>{part}</Fragment>
    ),
  );
}

export default function History({ modes }: { modes: ModesState | null }) {
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  // Se incrementa para recargar tras borrar.
  const [revision, setRevision] = useState(0);
  const reload = useCallback(() => setRevision((r) => r + 1), []);
  const [retryAvailable, setRetryAvailable] = useState(false);

  useEffect(() => {
    invoke<boolean>("dictation_retry_available")
      .then(setRetryAvailable)
      .catch(() => {});
    const unlisten = listen<boolean>("dictation-retry-available", (e) => {
      setRetryAvailable(e.payload);
      if (!e.payload) reload();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [reload]);

  useEffect(() => {
    let current = true;
    invoke<HistoryEntry[]>("history_list", { query })
      .then((list) => current && setEntries(list ?? []))
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

  const modeName = (id: string) =>
    id === "literal" ? "Literal" : (modes?.modes.find((m) => m.id === id)?.name ?? id);

  const groups = groupByDay(entries ?? []);
  const latestId = entries?.[0]?.id;

  return (
    <main className="page" style={{ gap: 16 }}>
      <div className="history-head">
        <h1 className="page-title">Historial</h1>
        <label className="search">
          <SearchIcon size={16} />
          <input
            type="search"
            aria-label="Buscar en el historial"
            placeholder="Buscar…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
      </div>
      {retryAvailable && (
        <div role="alert" className="alert alert-warn">
          <WarnIcon stroke="var(--warn)" />
          <span>El último dictado no se pudo procesar. El audio sigue en memoria.</span>
          <button
            type="button"
            className="btn btn-warn"
            onClick={() =>
              run(async () => {
                await invoke("dictation_retry");
                setRetryAvailable(false);
              }, "Reintentando…")
            }
          >
            Reintentar
          </button>
        </div>
      )}
      {entries !== null && entries.length === 0 && (
        <p className="empty">{query ? "Sin resultados." : "Todavía no hay dictados."}</p>
      )}
      {groups.map((g) => (
        <div key={g.label} className="stack-8">
          <span className="kicker-sm">{g.label}</span>
          <ul aria-label={`Dictados de ${g.label.toLowerCase()}`} className="entries">
            {g.entries.map((e) => {
              const latest = e.id === latestId;
              return (
                <li key={e.id} className={latest ? "entry latest" : "entry"}>
                  <div className="entry-body">
                    <p className="entry-text">{withCode(e.final_text)}</p>
                    <div className="entry-meta">
                      <span>{formatTime(e.created_at)}</span>
                      <span>·</span>
                      <span>{seconds(e.duration_ms)}</span>
                      <span className={latest ? "chip accent" : "chip"}>{modeName(e.mode)}</span>
                    </div>
                  </div>
                  <div className="entry-actions">
                    <button
                      type="button"
                      className="btn btn-xs"
                      onClick={() =>
                        run(() => invoke("history_deliver", { id: e.id, paste: false }), "Copiado")
                      }
                    >
                      Copiar
                    </button>
                    <button
                      type="button"
                      className="btn btn-xs"
                      onClick={() =>
                        run(() => invoke("history_deliver", { id: e.id, paste: true }), "Pegado")
                      }
                    >
                      Pegar de nuevo
                    </button>
                    <button
                      type="button"
                      className="btn btn-icon"
                      aria-label="Borrar"
                      onClick={() =>
                        run(async () => {
                          await invoke("history_delete", { id: e.id });
                          reload();
                        }, "Borrado")
                      }
                    >
                      <TrashIcon size={15} />
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
      {message && (
        <p role="status" className="toast">
          {message}
        </p>
      )}
    </main>
  );
}
