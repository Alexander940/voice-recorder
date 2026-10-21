import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MicIcon } from "./icons";
import type { AppSettings } from "./Settings";

export interface Available {
  version: string;
  notes: string | null;
}

export interface UpdateStatus {
  configured: boolean;
  enabled: boolean;
  available: Available | null;
  last_checked_ms?: number | null;
}

/** "hoy a las 09:00", "ayer a las 18:30" o la fecha. */
export function lastChecked(ms: number | null | undefined, now = Date.now()): string {
  if (!ms) return "Todavía no se ha comprobado";
  const time = new Date(ms).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
  const day = (t: number) => new Date(t).toDateString();
  if (day(ms) === day(now)) return `Última comprobación: hoy a las ${time}`;
  if (day(ms) === day(now - 86_400_000)) return `Última comprobación: ayer a las ${time}`;
  return `Última comprobación: ${new Date(ms).toLocaleDateString("es")} a las ${time}`;
}

/** Notas en líneas, sin viñetas de Markdown. */
export function noteLines(notes: string | null): string[] {
  return (notes ?? "")
    .split("\n")
    .map((l) => l.replace(/^\s*[-*]\s*/, "").trim())
    .filter(Boolean);
}

export default function Updates() {
  const [version, setVersion] = useState<string | null>(null);
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("app_version")
      .then(setVersion)
      .catch(() => {});
    invoke<UpdateStatus>("update_status")
      .then(setStatus)
      .catch(() => setStatus(null));
    const unlisten = listen<Available>("update-available", (e) =>
      setStatus((s) => (s ? { ...s, available: e.payload } : s)),
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function checkNow() {
    setChecking(true);
    setError(null);
    try {
      setStatus(await invoke<UpdateStatus>("update_check_now"));
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  }

  async function toggle(enabled: boolean) {
    setStatus((s) => (s ? { ...s, enabled } : s));
    try {
      const settings = await invoke<AppSettings>("settings_get");
      await invoke("settings_update", { settings: { ...settings, updates: { enabled } } });
    } catch (e) {
      setError(String(e));
      setStatus((s) => (s ? { ...s, enabled: !enabled } : s));
    }
  }

  async function install() {
    // Nunca se instala sin confirmación explícita.
    if (!window.confirm("Se descargará la actualización y la app se reiniciará. ¿Continuar?")) {
      return;
    }
    setInstalling(true);
    setError(null);
    try {
      await invoke("update_install");
    } catch (e) {
      setError(String(e));
      setInstalling(false);
    }
  }

  return (
    <main className="page">
      <h1 className="page-title">Actualizaciones</h1>
      <div className="version-card">
        <div className="app-tile">
          <MicIcon size={22} stroke="var(--bg)" />
        </div>
        <div className="version-text">
          <span className="version-name">Voice Recorder {version}</span>
          <span className="version-sub">
            {status && !status.configured
              ? "Las actualizaciones automáticas aún no están configuradas en esta compilación."
              : lastChecked(status?.last_checked_ms)}
          </span>
        </div>
        <button
          type="button"
          className="btn btn-sm"
          disabled={checking || !status?.configured}
          onClick={checkNow}
        >
          {checking ? "Buscando…" : "Buscar ahora"}
        </button>
      </div>
      {status?.available && (
        <div role="status" className="update-card">
          <div className="update-top">
            <div className="version-text">
              <span className="update-kicker">Nueva versión disponible</span>
              <span className="update-version">{status.available.version}</span>
            </div>
            <button
              type="button"
              className="btn btn-primary pad-18"
              onClick={install}
              disabled={installing}
            >
              {installing ? "Instalando…" : "Instalar y reiniciar"}
            </button>
          </div>
          {noteLines(status.available.notes).length > 0 && (
            <ul className="update-notes">
              {noteLines(status.available.notes).map((n, i) => (
                <li key={i}>{n}</li>
              ))}
            </ul>
          )}
          <span className="update-note">
            Se pedirá confirmación antes de descargar. La app se reiniciará al terminar.
          </span>
        </div>
      )}
      {error && (
        <p role="alert" className="page-lead error-text">
          {error}
        </p>
      )}
      <label className="setting toggle-card">
        <span className="setting-text">
          <span className="setting-name">Buscar actualizaciones automáticamente</span>
          <span className="setting-desc">
            Al iniciar y una vez al día. Nunca se instala nada sin tu permiso.
          </span>
        </span>
        <input
          type="checkbox"
          role="switch"
          className="switch-input"
          checked={status?.enabled ?? true}
          onChange={(e) => toggle(e.target.checked)}
        />
        <span aria-hidden="true" className="switch">
          <span className="switch-knob" />
        </span>
      </label>
    </main>
  );
}
