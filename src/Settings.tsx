import { Fragment, useEffect, useState, type KeyboardEvent, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isMac } from "./platform";

export interface AppSettings {
  version: number;
  general: { autostart: boolean };
  hotkey: { shortcut: string; mode: "hold" | "toggle" };
  audio: { device: string | null; max_duration_secs: number };
  transcription: { model: string; language: string | null };
  ai: { cleanup: boolean };
  output: { copy_only: boolean };
  privacy: { enabled: boolean; retention_days: number };
  feedback: { sounds: boolean };
  updates: { enabled: boolean };
}

const MODELS = ["whisper-large-v3-turbo", "whisper-large-v3"];
const LANGUAGES: [string, string][] = [
  ["", "Detectar automáticamente"],
  ["es", "Español"],
  ["en", "Inglés"],
  ["pt", "Portugués"],
  ["fr", "Francés"],
  ["de", "Alemán"],
  ["it", "Italiano"],
];

const MODIFIER_KEYS = ["Control", "Alt", "Shift", "Meta"];

/** Pulsación de teclado → atajo con el formato del backend ("Option+Space", "Ctrl+Alt+K"). */
export function shortcutFromEvent(
  e: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">,
  mac = isMac(),
): string | null {
  if (MODIFIER_KEYS.includes(e.key)) return null;
  const mods: string[] = [];
  if (mac) {
    if (e.metaKey) mods.push("Cmd");
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Option");
  } else {
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.metaKey) mods.push("Super");
  }
  if (e.shiftKey) mods.push("Shift");
  if (mods.length === 0) return null;
  const key = e.code.startsWith("Key")
    ? e.code.slice(3)
    : e.code.startsWith("Digit")
      ? e.code.slice(5)
      : e.code;
  return [...mods, key].join("+");
}

function Switch({
  name,
  desc,
  checked,
  onChange,
}: {
  name: string;
  desc?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="setting">
      {desc ? (
        <span className="setting-text">
          <span className="setting-name">{name}</span>
          <span className="setting-desc">{desc}</span>
        </span>
      ) : (
        <span className="setting-name">{name}</span>
      )}
      <input
        type="checkbox"
        role="switch"
        className="switch-input"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span aria-hidden="true" className="switch">
        <span className="switch-knob" />
      </span>
    </label>
  );
}

function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section aria-labelledby={id} className="settings-section">
      <h2 id={id} className="kicker">
        {title}
      </h2>
      <div className="card">{children}</div>
    </section>
  );
}

export default function Settings({ onSaved }: { onSaved?: (s: AppSettings) => void }) {
  const [saved, setSaved] = useState<AppSettings | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [devices, setDevices] = useState<string[]>([]);
  const [capturing, setCapturing] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    invoke<AppSettings>("settings_get").then((s) => {
      setSaved(s);
      setSettings(s);
    });
    invoke<string[]>("list_input_devices")
      .then((d) => setDevices(d ?? []))
      .catch(() => setDevices([]));
  }, []);

  if (!settings || !saved) return <main className="page">Cargando ajustes…</main>;

  const dirty = JSON.stringify(settings) !== JSON.stringify(saved);

  // Actualiza una sección sin tocar las demás.
  function set<K extends keyof AppSettings>(section: K, patch: Partial<AppSettings[K]>) {
    setStatus(null);
    setSettings((s) => (s ? { ...s, [section]: { ...(s[section] as object), ...patch } } : s));
  }

  async function save() {
    try {
      const next = await invoke<AppSettings>("settings_update", { settings });
      setSaved(next);
      setSettings(next);
      onSaved?.(next);
      setStatus({ ok: true, text: "Ajustes guardados" });
    } catch (e) {
      setStatus({ ok: false, text: String(e) });
    }
  }

  function onCaptureKey(e: KeyboardEvent<HTMLButtonElement>) {
    if (!capturing) return;
    e.preventDefault();
    if (e.key === "Escape") {
      setCapturing(false);
      return;
    }
    const shortcut = shortcutFromEvent(e);
    if (shortcut) {
      set("hotkey", { shortcut });
      setCapturing(false);
    }
  }

  const keys = settings.hotkey.shortcut.split("+");
  const longLimit = settings.audio.max_duration_secs > 120 * 60;

  return (
    <main className="page" style={{ gap: 22 }}>
      <h1 className="page-title">Ajustes</h1>

      <Section id="s-hotkey" title="Atajo">
        <div className="setting">
          <div className="setting-text">
            <span className="setting-name">Atajo de dictado</span>
            <span className="setting-desc">Funciona en cualquier aplicación. Esc cancela.</span>
          </div>
          <div className="shortcut">
            {capturing ? (
              <span className="capturing">Pulsa la combinación…</span>
            ) : (
              keys.map((k, i) => (
                <Fragment key={i}>
                  {i > 0 && <span className="plus">+</span>}
                  <span className="kbd">{k}</span>
                </Fragment>
              ))
            )}
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => setCapturing(!capturing)}
              onKeyDown={onCaptureKey}
              onBlur={() => setCapturing(false)}
            >
              {capturing ? "Cancelar" : "Cambiar"}
            </button>
          </div>
        </div>
        <fieldset className="setting">
          <legend>Modo del atajo</legend>
          <div className="segmented">
            {(
              [
                ["toggle", "Pulsar para empezar y terminar"],
                ["hold", "Mantener pulsado"],
              ] as const
            ).map(([value, label]) => (
              <label
                key={value}
                className={settings.hotkey.mode === value ? "segment selected" : "segment"}
              >
                <input
                  type="radio"
                  name="hotkey-mode"
                  checked={settings.hotkey.mode === value}
                  onChange={() => set("hotkey", { mode: value })}
                />
                {label}
              </label>
            ))}
          </div>
        </fieldset>
      </Section>

      <Section id="s-audio" title="Audio y transcripción">
        <label className="setting compact labelled">
          Micrófono
          <select
            className="select"
            value={settings.audio.device ?? ""}
            onChange={(e) => set("audio", { device: e.target.value || null })}
          >
            <option value="">Predeterminado del sistema</option>
            {devices.map((d) => (
              <option key={d} value={d}>
                {d}
              </option>
            ))}
          </select>
        </label>
        <label className="setting compact labelled">
          <span className="setting-text">
            Duración máxima
            <span className={longLimit ? "setting-desc warn-text" : "setting-desc"}>
              {longLimit
                ? "Más de 2 h seguidas pueden agotar tu límite de audio por hora de Groq (plan gratuito)."
                : "La grabación se corta y se transcribe al llegar al límite."}
            </span>
          </span>
          <span className="unit-input">
            <input
              type="number"
              className="number"
              min={1}
              max={180}
              value={Math.round(settings.audio.max_duration_secs / 60)}
              onChange={(e) => set("audio", { max_duration_secs: Number(e.target.value) * 60 })}
            />
            min
          </span>
        </label>
        <label className="setting compact labelled">
          Modelo de Whisper
          <select
            className="select mono"
            value={settings.transcription.model}
            onChange={(e) => set("transcription", { model: e.target.value })}
          >
            {MODELS.map((m) => (
              <option key={m}>{m}</option>
            ))}
          </select>
        </label>
        <label className="setting compact labelled">
          Idioma
          <select
            className="select"
            value={settings.transcription.language ?? ""}
            onChange={(e) => set("transcription", { language: e.target.value || null })}
          >
            {LANGUAGES.map(([code, name]) => (
              <option key={code} value={code}>
                {name}
              </option>
            ))}
          </select>
        </label>
      </Section>

      <Section id="s-ai" title="Texto y salida">
        <Switch
          name="Limpiar el texto con Claude"
          desc="Quita muletillas y aplica el modo activo. Si falla, se pega el texto sin limpiar."
          checked={settings.ai.cleanup}
          onChange={(v) => set("ai", { cleanup: v })}
        />
        <Switch
          name="Solo copiar al portapapeles"
          desc="No pega automáticamente; tú decides dónde."
          checked={settings.output.copy_only}
          onChange={(v) => set("output", { copy_only: v })}
        />
        <Switch
          name="Sonidos"
          desc="Al empezar, al terminar y en errores."
          checked={settings.feedback.sounds}
          onChange={(v) => set("feedback", { sounds: v })}
        />
      </Section>

      <Section id="s-general" title="General y privacidad">
        <Switch
          name="Iniciar con el sistema"
          checked={settings.general.autostart}
          onChange={(v) => set("general", { autostart: v })}
        />
        <Switch
          name="Guardar historial de dictados"
          checked={settings.privacy.enabled}
          onChange={(v) => set("privacy", { enabled: v })}
        />
        <label className="setting compact labelled">
          <span className="setting-text">
            Conservar el historial
            <span className="setting-desc">0 = para siempre. Solo se guarda en este equipo.</span>
          </span>
          <span className="unit-input">
            <input
              type="number"
              className="number"
              min={0}
              value={settings.privacy.retention_days}
              disabled={!settings.privacy.enabled}
              onChange={(e) => set("privacy", { retention_days: Number(e.target.value) })}
            />
            días
          </span>
        </label>
      </Section>

      <div className="grow" />
      <div className="save-bar ruled">
        {dirty ? (
          <span className="save-status">Hay cambios sin guardar</span>
        ) : (
          status && (
            <span
              role={status.ok ? "status" : "alert"}
              className={status.ok ? "save-status ok-text" : "save-status error-text"}
            >
              {status.text}
            </span>
          )
        )}
        {dirty && status && !status.ok && (
          <span role="alert" className="save-status error-text">
            {status.text}
          </span>
        )}
        <button
          type="button"
          className="btn btn-secondary"
          disabled={!dirty}
          onClick={() => {
            setSettings(saved);
            setStatus(null);
          }}
        >
          Descartar
        </button>
        <button type="button" className="btn btn-primary" disabled={!dirty} onClick={save}>
          Guardar ajustes
        </button>
      </div>
    </main>
  );
}
