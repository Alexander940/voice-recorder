import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckIcon, KeyboardIcon, LockIcon, MicIcon } from "./icons";
import { KeyErrorAlert, PROVIDERS, openSite, type KeyStatus, type Provider } from "./keys";
import { defaultShortcut, isMac, seconds } from "./platform";

export type { KeyStatus, Provider } from "./keys";

export const STEPS = ["welcome", "groq", "anthropic", "permissions", "test"] as const;
export type Step = (typeof STEPS)[number];

export interface DictationResult {
  text: string;
  mode: string;
  audio_ms: number;
  processing_ms: number;
}

function Progress({ step }: { step: Step }) {
  const index = STEPS.indexOf(step);
  return (
    <div className="ob-progress" aria-label={`Paso ${index + 1} de ${STEPS.length}`}>
      {STEPS.map((s, i) => (
        <span
          key={s}
          className={i === index ? "ob-dot current" : i < index ? "ob-dot done" : "ob-dot"}
        />
      ))}
      <span className="ob-progress-text">
        Paso {index + 1} de {STEPS.length}
      </span>
    </div>
  );
}

function Frame({ step, body, actions }: { step: Step; body: ReactNode; actions: ReactNode }) {
  return (
    <div className="onboarding-frame">
      <main className="onboarding">
        {body}
        <div className="ob-foot">
          <Progress step={step} />
          <div className="ob-actions">{actions}</div>
        </div>
      </main>
    </div>
  );
}

function KeyStepBody({
  provider,
  kicker,
  help,
  keyValue,
  setKey,
  error,
}: {
  provider: Provider;
  kicker: string;
  help: ReactNode;
  keyValue: string;
  setKey: (k: string) => void;
  error: string | null;
}) {
  const p = PROVIDERS[provider];
  const id = `${provider}-key`;
  return (
    <div className="ob-body roomy">
      <div className="ob-head">
        <span className="kicker-sm">{kicker}</span>
        <h1 className="ob-title">API key de {p.name}</h1>
        <p className="ob-text">{help}</p>
      </div>
      <div className="ob-field">
        <label htmlFor={id}>API key</label>
        <input
          id={id}
          type="password"
          autoComplete="off"
          className={error ? "input key invalid" : "input key"}
          placeholder={`${p.prefix}…`}
          value={keyValue}
          onChange={(e) => setKey(e.target.value)}
        />
        <div className="ob-note">
          <LockIcon size={16} stroke="var(--muted)" />
          Se guarda en el llavero del sistema. Nunca en texto plano.
        </div>
      </div>
      {error && <KeyErrorAlert provider={provider} error={error} />}
    </div>
  );
}

function SiteLink({ provider }: { provider: Provider }) {
  const site = PROVIDERS[provider].site;
  return (
    <a
      href={`https://${site}`}
      onClick={(e) => {
        e.preventDefault();
        openSite(provider);
      }}
    >
      {site}
    </a>
  );
}

export default function Onboarding({
  onDone,
  initialStep = "welcome",
}: {
  onDone: () => void;
  initialStep?: Step;
}) {
  const [step, setStep] = useState<Step>(initialStep);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<DictationResult | null>(null);
  const mac = isMac();
  const shortcut = defaultShortcut();

  const go = (delta: number) => {
    setKey("");
    setError(null);
    setStep(STEPS[STEPS.indexOf(step) + delta]);
  };

  useEffect(() => {
    if (step !== "test") return;
    const unlisten = listen<DictationResult>("dictation-result", (e) => setResult(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [step]);

  async function saveKey(provider: Provider) {
    setBusy(true);
    setError(null);
    try {
      await invoke<KeyStatus>("save_api_key", { provider, key });
      // La key no se conserva en el frontend una vez guardada.
      go(1);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const back = (
    <button type="button" className="btn btn-secondary" onClick={() => go(-1)} disabled={busy}>
      Atrás
    </button>
  );

  if (step === "welcome") {
    return (
      <Frame
        step={step}
        body={
          <div className="ob-body center">
            <div className="ob-mark">
              <MicIcon size={34} stroke="var(--bg)" />
            </div>
            <div className="ob-head">
              <h1 className="ob-title big">Bienvenido a Voice Recorder</h1>
              <p className="ob-text">
                Dicta en cualquier aplicación. El texto aparece donde está el cursor, limpio y con
                formato.
              </p>
            </div>
            <div className="ob-steps">
              <div className="ob-step">
                <span className="kbd">{shortcut}</span>
                <span className="ob-step-label">1. Pulsa el atajo</span>
              </div>
              <div className="ob-step">
                <svg width="56" height="22" viewBox="0 0 56 22" aria-hidden="true">
                  <g style={{ fill: "var(--rec)" }}>
                    <rect x="2" y="8" width="4" height="6" rx="2" />
                    <rect x="10" y="4" width="4" height="14" rx="2" />
                    <rect x="18" y="1" width="4" height="20" rx="2" />
                    <rect x="26" y="6" width="4" height="10" rx="2" />
                    <rect x="34" y="3" width="4" height="16" rx="2" />
                    <rect x="42" y="7" width="4" height="8" rx="2" />
                    <rect x="50" y="9" width="4" height="4" rx="2" />
                  </g>
                </svg>
                <span className="ob-step-label">2. Habla</span>
              </div>
              <div className="ob-step">
                <span className="kbd">{shortcut}</span>
                <span className="ob-step-label">3. Púlsalo otra vez</span>
              </div>
            </div>
          </div>
        }
        actions={
          <button type="button" className="btn btn-primary" onClick={() => go(1)}>
            Empezar
          </button>
        }
      />
    );
  }

  if (step === "groq" || step === "anthropic") {
    const provider = step;
    return (
      <Frame
        step={step}
        body={
          <KeyStepBody
            provider={provider}
            kicker={provider === "groq" ? "Transcripción" : "Limpieza con IA"}
            help={
              provider === "groq" ? (
                <>
                  Se usa para transcribir tu voz con Whisper. Créala gratis en{" "}
                  <SiteLink provider="groq" /> (con q).
                </>
              ) : (
                <>
                  Se usa para limpiar y dar formato al texto con Claude. Créala en{" "}
                  <SiteLink provider="anthropic" />.
                </>
              )
            }
            keyValue={key}
            setKey={setKey}
            error={error}
          />
        }
        actions={
          <>
            {back}
            {provider === "anthropic" && (
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => go(1)}
                disabled={busy}
              >
                Omitir por ahora
              </button>
            )}
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => saveKey(provider)}
              disabled={busy || key.trim() === ""}
            >
              {busy ? "Validando…" : "Validar y guardar"}
            </button>
          </>
        }
      />
    );
  }

  if (step === "permissions") {
    return (
      <Frame
        step={step}
        body={
          mac ? (
            <div className="ob-body">
              <div className="ob-head">
                <span className="kicker-sm">macOS</span>
                <h1 className="ob-title">Dos permisos y listo</h1>
                <p className="ob-text">
                  Voice Recorder solo escucha mientras grabas y solo escribe cuando pega el texto.
                </p>
              </div>
              <div className="perm-list">
                <div className="perm">
                  <div className="perm-icon">
                    <MicIcon size={20} stroke="var(--accent)" />
                  </div>
                  <div className="perm-text">
                    <span className="perm-name">Micrófono</span>
                    <span className="perm-desc">macOS lo pedirá en tu primer dictado.</span>
                  </div>
                </div>
                <div className="perm">
                  <div className="perm-icon">
                    <KeyboardIcon size={20} stroke="var(--accent)" />
                  </div>
                  <div className="perm-text">
                    <span className="perm-name">Accesibilidad</span>
                    <span className="perm-desc">
                      Para pegar el texto. Ajustes del Sistema → Privacidad y seguridad →
                      Accesibilidad.
                    </span>
                  </div>
                  <button
                    type="button"
                    className="btn btn-sm"
                    onClick={() => openSite("accessibility")}
                  >
                    Abrir Ajustes
                  </button>
                </div>
              </div>
              <p className="ob-foot-note">
                En Windows y Linux no hace falta ningún permiso adicional.
              </p>
            </div>
          ) : (
            <div className="ob-body">
              <div className="ob-head">
                <span className="kicker-sm">Permisos</span>
                <h1 className="ob-title">Sin permisos adicionales</h1>
                <p className="ob-text">
                  En este sistema Voice Recorder puede usar el micrófono y pegar el texto sin
                  pedirte nada más.
                </p>
              </div>
            </div>
          )
        }
        actions={
          <>
            {back}
            <button type="button" className="btn btn-primary" onClick={() => go(1)}>
              Continuar
            </button>
          </>
        }
      />
    );
  }

  return (
    <Frame
      step={step}
      body={
        <div className="ob-body">
          <div className="ob-head">
            <span className="kicker-sm">Último paso</span>
            <h1 className="ob-title">Prueba tu primer dictado</h1>
            <p className="ob-text loose">
              Pulsa <kbd className="kbd-inline">{shortcut}</kbd>, di algo y vuelve a pulsarlo para
              terminar.
            </p>
          </div>
          {result ? (
            <>
              <div className="result">
                <div className="result-head">
                  <span className="result-label">Resultado</span>
                  <span className="result-chip">Modo {result.mode}</span>
                </div>
                <blockquote className="result-text" data-testid="transcript">
                  {result.text}
                </blockquote>
                <span className="result-meta">
                  {seconds(result.audio_ms)} de audio · procesado en {seconds(result.processing_ms)}
                </span>
              </div>
              <div className="success-line">
                <CheckIcon size={18} stroke="var(--ok)" />
                Todo funciona. Puedes dictar en cualquier aplicación.
              </div>
            </>
          ) : (
            <div className="result">
              <p className="result-waiting">Esperando tu dictado…</p>
            </div>
          )}
        </div>
      }
      actions={
        <button type="button" className="btn btn-primary" onClick={onDone}>
          {result ? "Terminar" : "Saltar prueba"}
        </button>
      }
    />
  );
}
