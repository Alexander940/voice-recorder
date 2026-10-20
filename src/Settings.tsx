import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Modes from "./Modes";
import Vocabulary from "./Vocabulary";
import { KeyStep, type KeyStatus, type Provider } from "./Onboarding";

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
}

const MODELS = ["whisper-large-v3-turbo", "whisper-large-v3"];
const LANGUAGES: [string, string][] = [
  ["", "Detección automática"],
  ["es", "Español"],
  ["en", "Inglés"],
  ["pt", "Portugués"],
  ["fr", "Francés"],
  ["de", "Alemán"],
  ["it", "Italiano"],
];
const PROVIDERS: [Provider, string, string][] = [
  ["groq", "Groq", "gsk_"],
  ["anthropic", "Anthropic", "sk-ant-"],
];

export default function Settings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [devices, setDevices] = useState<string[]>([]);
  const [keys, setKeys] = useState<KeyStatus | null>(null);
  const [editingKey, setEditingKey] = useState<Provider | null>(null);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    invoke<AppSettings>("settings_get").then(setSettings);
    invoke<string[]>("list_input_devices")
      .then(setDevices)
      .catch(() => setDevices([]));
    invoke<KeyStatus>("get_key_status").then(setKeys);
  }, []);

  if (!settings) return <p>Cargando ajustes…</p>;

  // Actualiza una sección sin tocar las demás.
  function set<K extends keyof AppSettings>(section: K, patch: Partial<AppSettings[K]>) {
    setSettings((s) => (s ? { ...s, [section]: { ...(s[section] as object), ...patch } } : s));
  }

  async function save() {
    try {
      setSettings(await invoke<AppSettings>("settings_update", { settings }));
      setStatus({ ok: true, text: "Ajustes guardados" });
    } catch (e) {
      setStatus({ ok: false, text: String(e) });
    }
  }

  async function deleteKey(provider: Provider) {
    setKeys(await invoke<KeyStatus>("delete_api_key", { provider }));
  }

  return (
    <div>
      <section aria-labelledby="s-general">
        <h3 id="s-general">General</h3>
        <label>
          <input
            type="checkbox"
            checked={settings.general.autostart}
            onChange={(e) => set("general", { autostart: e.target.checked })}
          />
          Iniciar con el sistema
        </label>
        <label>
          <input
            type="checkbox"
            checked={settings.feedback.sounds}
            onChange={(e) => set("feedback", { sounds: e.target.checked })}
          />
          Sonidos al empezar, terminar y en errores
        </label>
      </section>

      <section aria-labelledby="s-hotkey">
        <h3 id="s-hotkey">Atajos</h3>
        <label>
          Atajo de dictado
          <input
            value={settings.hotkey.shortcut}
            onChange={(e) => set("hotkey", { shortcut: e.target.value })}
          />
        </label>
        <label>
          Modo del atajo
          <select
            value={settings.hotkey.mode}
            onChange={(e) => set("hotkey", { mode: e.target.value as "hold" | "toggle" })}
          >
            <option value="hold">Mantener pulsado</option>
            <option value="toggle">Pulsar para empezar y terminar</option>
          </select>
        </label>
      </section>

      <section aria-labelledby="s-audio">
        <h3 id="s-audio">Audio</h3>
        <label>
          Micrófono
          <select
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
        <label>
          Duración máxima (minutos)
          <input
            type="number"
            min={1}
            max={30}
            value={Math.round(settings.audio.max_duration_secs / 60)}
            onChange={(e) => set("audio", { max_duration_secs: Number(e.target.value) * 60 })}
          />
        </label>
      </section>

      <section aria-labelledby="s-stt">
        <h3 id="s-stt">Transcripción</h3>
        <label>
          Modelo
          <select
            value={settings.transcription.model}
            onChange={(e) => set("transcription", { model: e.target.value })}
          >
            {MODELS.map((m) => (
              <option key={m}>{m}</option>
            ))}
          </select>
        </label>
        <label>
          Idioma
          <select
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
      </section>

      <section aria-labelledby="s-ai">
        <h3 id="s-ai">IA</h3>
        <label>
          <input
            type="checkbox"
            checked={settings.ai.cleanup}
            onChange={(e) => set("ai", { cleanup: e.target.checked })}
          />
          Limpiar el texto con Claude
        </label>
        <label>
          <input
            type="checkbox"
            checked={settings.output.copy_only}
            onChange={(e) => set("output", { copy_only: e.target.checked })}
          />
          Solo copiar al portapapeles (no pegar)
        </label>
        <Modes />
        <Vocabulary />
      </section>

      <section aria-labelledby="s-accounts">
        <h3 id="s-accounts">Cuentas</h3>
        <ul aria-label="API keys">
          {PROVIDERS.map(([p, name, prefix]) => (
            <li key={p}>
              {name}: {keys?.[p] ? "configurada" : "sin configurar"}
              <button onClick={() => setEditingKey(p)}>{keys?.[p] ? "Cambiar" : "Añadir"}</button>
              {keys?.[p] && <button onClick={() => deleteKey(p)}>Borrar</button>}
              {editingKey === p && (
                <KeyStep
                  provider={p}
                  title={`API key de ${name}`}
                  prefix={prefix}
                  help="Se guarda en el almacén seguro del sistema."
                  onSaved={(s) => {
                    setKeys(s);
                    setEditingKey(null);
                  }}
                />
              )}
            </li>
          ))}
        </ul>
      </section>

      <section aria-labelledby="s-privacy">
        <h3 id="s-privacy">Privacidad</h3>
        <label>
          <input
            type="checkbox"
            checked={settings.privacy.enabled}
            onChange={(e) => set("privacy", { enabled: e.target.checked })}
          />
          Guardar historial de dictados
        </label>
        <label>
          Conservar (días, 0 = siempre)
          <input
            type="number"
            min={0}
            value={settings.privacy.retention_days}
            disabled={!settings.privacy.enabled}
            onChange={(e) => set("privacy", { retention_days: Number(e.target.value) })}
          />
        </label>
      </section>

      <button onClick={save}>Guardar ajustes</button>
      {status && <p role={status.ok ? "status" : "alert"}>{status.text}</p>}
    </div>
  );
}
