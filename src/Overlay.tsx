import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./overlay.css";

export interface LevelEvent {
  level: number;
  elapsed_ms: number;
}

const BARS = 12;

export function formatElapsed(ms: number): string {
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export default function Overlay() {
  const [level, setLevel] = useState(0);
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    const unlisten = listen<LevelEvent>("audio-level", (e) => {
      setLevel(e.payload.level);
      setElapsed(e.payload.elapsed_ms);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const active = Math.round(level * BARS);

  return (
    <div className="overlay" role="status" aria-label="Grabando">
      <span className="overlay-dot" />
      <div className="overlay-bars" data-testid="level" data-active={active}>
        {Array.from({ length: BARS }, (_, i) => (
          <span key={i} className={i < active ? "bar on" : "bar"} />
        ))}
      </div>
      <span className="overlay-time" data-testid="elapsed">
        {formatElapsed(elapsed)}
      </span>
    </div>
  );
}
