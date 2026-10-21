import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckIcon, CloseIcon } from "./icons";
import { seconds } from "./platform";
import "./styles/overlay.css";

export interface LevelEvent {
  level: number;
  elapsed_ms: number;
}

export type OverlayState =
  | { kind: "recording"; mode: string; shortcut: string; toggle: boolean }
  | { kind: "processing"; stage: "transcribing" | "cleaning"; mode: string }
  | { kind: "pasted"; words: number; ms: number; copied: boolean }
  | { kind: "error"; title: string; detail: string; retry: boolean };

const BARS = 12;
/** Margen transparente alrededor de la píldora para que quepa la sombra. */
const PADDING = 16;

export function formatElapsed(ms: number): string {
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = s.toString().padStart(2, "0");
  return h > 0 ? `${h}:${m.toString().padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** Forma de onda de los últimos niveles; sin datos todavía, un punto. */
export function barHeights(history: number[]): (number | null)[] {
  return Array.from({ length: BARS }, (_, i) => {
    const v = history[i];
    return v === undefined ? null : Math.max(4, Math.round(Math.min(1, Math.max(0, v)) * 22));
  });
}

function Bars({ history }: { history: number[] }) {
  const heights = barHeights(history);
  return (
    <svg
      width="92"
      height="22"
      viewBox="0 0 92 22"
      aria-hidden="true"
      data-testid="level"
      data-active={heights.filter((h) => h !== null).length}
    >
      {heights.map((h, i) => (
        <rect
          key={i}
          x={i * 8}
          y={(22 - (h ?? 4)) / 2}
          width="4"
          height={h ?? 4}
          rx="2"
          className={h === null ? "bar" : "bar on"}
        />
      ))}
    </svg>
  );
}

export default function Overlay({ initial = null }: { initial?: OverlayState | null }) {
  const [state, setState] = useState<OverlayState | null>(initial);
  const [history, setHistory] = useState<number[]>([]);
  const [elapsed, setElapsed] = useState(0);
  // Trozos de un dictado largo ya transcritos mientras se graba.
  const [chunks, setChunks] = useState(0);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const subs = [
      listen<OverlayState>("overlay-state", (e) => {
        setState(e.payload);
        if (e.payload.kind === "recording") {
          setElapsed(0);
          setHistory([]);
          setChunks(0);
        }
      }),
      listen<number>("dictation-progress", (e) => setChunks(e.payload)),
      listen<LevelEvent>("audio-level", (e) => {
        setElapsed(e.payload.elapsed_ms);
        setHistory((h) => [...h.slice(-(BARS - 1)), e.payload.level]);
      }),
    ];
    return () => {
      subs.forEach((s) => s.then((fn) => fn()));
    };
  }, []);

  // La ventana nativa se ajusta al contenido para no tapar clics alrededor.
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const fit = () =>
      invoke("overlay_fit", {
        width: Math.ceil(el.offsetWidth) + PADDING * 2,
        height: Math.ceil(el.offsetHeight) + PADDING * 2,
      }).catch(() => {});
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [state?.kind]);

  if (!state) return null;

  return (
    <div className="overlay-root">
      <div className="overlay-stack" ref={root}>
        {state.kind === "recording" && (
          <>
            <div role="status" aria-label="Grabando" className="pill recording" data-overlay-pill>
              <span className="rec-dot" />
              <Bars history={history} />
              <span className="pill-chip" data-testid="mode">
                {state.mode}
              </span>
              <span className="pill-time" data-testid="elapsed">
                {formatElapsed(elapsed)}
              </span>
              {chunks > 0 && (
                <span className="pill-sub" data-testid="chunks">
                  {chunks} {chunks === 1 ? "trozo transcrito" : "trozos transcritos"}
                </span>
              )}
              <button
                type="button"
                className="pill-close"
                aria-label="Cancelar grabación"
                onClick={() => invoke("dictation_cancel").catch(() => {})}
              >
                <CloseIcon size={14} />
              </button>
            </div>
            <span className="overlay-hint" data-overlay-hint>
              {state.toggle ? "Pulsa " : "Suelta "}
              <span className="mono">{state.shortcut}</span>
              {" para terminar · Esc para cancelar"}
            </span>
          </>
        )}
        {state.kind === "processing" && (
          <div role="status" aria-label="Procesando" className="pill processing" data-overlay-pill>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <circle cx="12" cy="12" r="9" className="spin-track" strokeWidth="3" />
              <path
                d="M21 12a9 9 0 0 0-9-9"
                className="spin-head"
                strokeWidth="3"
                strokeLinecap="round"
              />
            </svg>
            <span className="pill-title">
              {state.stage === "cleaning" ? "Puliendo con Claude…" : "Transcribiendo…"}
            </span>
            <span className="pill-chip">{state.mode}</span>
          </div>
        )}
        {state.kind === "pasted" && (
          <div role="status" aria-label="Pegado" className="pill pasted" data-overlay-pill>
            <span className="pill-badge ok">
              <CheckIcon size={14} stroke="var(--bg)" width={3} />
            </span>
            <span className="pill-title">{state.copied ? "Copiado" : "Pegado"}</span>
            <span className="pill-sub">
              {state.words} {state.words === 1 ? "palabra" : "palabras"} · {seconds(state.ms)}
            </span>
          </div>
        )}
        {state.kind === "error" && (
          <div role="alert" className="pill error" data-overlay-pill>
            <span className="pill-badge danger">!</span>
            <span className="pill-title">{state.title}</span>
            {state.detail && <span className="pill-sub">{state.detail}</span>}
            {state.retry ? (
              <button
                type="button"
                className="pill-action"
                onClick={() => invoke("dictation_retry").catch(() => {})}
              >
                Reintentar
              </button>
            ) : (
              <button
                type="button"
                className="pill-close"
                aria-label="Cerrar"
                onClick={() => invoke("overlay_dismiss").catch(() => {})}
              >
                <CloseIcon size={14} />
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
