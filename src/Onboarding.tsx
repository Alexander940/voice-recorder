import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Provider = "groq" | "anthropic";

export interface KeyStatus {
  groq: boolean;
  anthropic: boolean;
}

export const STEPS = ["welcome", "groq", "anthropic", "permissions", "test"] as const;
export type Step = (typeof STEPS)[number];

const isMac = () => navigator.userAgent.includes("Mac");

interface KeyStepProps {
  provider: Provider;
  title: string;
  prefix: string;
  help: string;
  optional?: boolean;
  onSaved: (status: KeyStatus) => void;
  onSkip?: () => void;
}

export function KeyStep({
  provider,
  title,
  prefix,
  help,
  optional,
  onSaved,
  onSkip,
}: KeyStepProps) {
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function save() {
    setBusy(true);
    setError(null);
    try {
      const status = await invoke<KeyStatus>("save_api_key", { provider, key });
      // La key no se conserva en el frontend una vez guardada.
      setKey("");
      onSaved(status);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section>
      <h2>{title}</h2>
      <p>{help}</p>
      <label>
        API key
        <input
          type="password"
          autoComplete="off"
          placeholder={`${prefix}…`}
          value={key}
          onChange={(e) => setKey(e.target.value)}
        />
      </label>
      {error && <p role="alert">{error}</p>}
      <button onClick={save} disabled={busy || key.trim() === ""}>
        {busy ? "Validando…" : "Validar y guardar"}
      </button>
      {optional && onSkip && (
        <button onClick={onSkip} disabled={busy}>
          Omitir por ahora
        </button>
      )}
    </section>
  );
}

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<Step>("welcome");
  const [transcript, setTranscript] = useState<string | null>(null);
  const next = () => setStep(STEPS[STEPS.indexOf(step) + 1]);

  useEffect(() => {
    if (step !== "test") return;
    const unlisten = listen<string>("transcription", (e) => setTranscript(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [step]);

  return (
    <div>
      <p aria-label="progreso">
        Paso {STEPS.indexOf(step) + 1} de {STEPS.length}
      </p>
      {step === "welcome" && (
        <section>
          <h2>Bienvenido a Voice Recorder</h2>
          <p>Dicta en cualquier aplicación: mantén el atajo, habla y suelta.</p>
          <button onClick={next}>Empezar</button>
        </section>
      )}
      {step === "groq" && (
        <KeyStep
          provider="groq"
          title="API key de Groq"
          prefix="gsk_"
          help="Se usa para transcribir tu voz. Créala en console.groq.com (con q)."
          onSaved={next}
        />
      )}
      {step === "anthropic" && (
        <KeyStep
          provider="anthropic"
          title="API key de Anthropic"
          prefix="sk-ant-"
          help="Se usa para limpiar y dar formato al texto con Claude. Créala en console.anthropic.com."
          optional
          onSaved={next}
          onSkip={next}
        />
      )}
      {step === "permissions" && (
        <section>
          <h2>Permisos</h2>
          {isMac() ? (
            <p>
              macOS pedirá acceso al <strong>micrófono</strong> en tu primer dictado. Para pegar el
              texto, activa Voice Recorder en Ajustes del Sistema → Privacidad y seguridad →{" "}
              <strong>Accesibilidad</strong>.
            </p>
          ) : (
            <p>No hacen falta permisos adicionales en este sistema.</p>
          )}
          <button onClick={next}>Continuar</button>
        </section>
      )}
      {step === "test" && (
        <section>
          <h2>Prueba de dictado</h2>
          <p>
            Mantén <kbd>{isMac() ? "Option+Space" : "Ctrl+Alt+Space"}</kbd>, di algo y suelta.
          </p>
          {transcript !== null && <blockquote data-testid="transcript">{transcript}</blockquote>}
          <button onClick={onDone}>{transcript !== null ? "Terminar" : "Saltar prueba"}</button>
        </section>
      )}
    </div>
  );
}
