import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LockIcon } from "./icons";
import {
  KeyErrorAlert,
  PROVIDERS,
  openSite,
  type KeyHints,
  type KeyStatus,
  type Provider,
} from "./keys";

const DESCRIPTION: Record<Provider, { short: string; long: string }> = {
  groq: {
    short: "Transcripción con Whisper",
    long: "Transcripción con Whisper. Obligatoria para dictar.",
  },
  anthropic: {
    short: "Limpieza y modos con Claude",
    long: "Limpieza y modos con Claude. Opcional: sin ella se pega el texto tal cual.",
  },
};

function AccountCard({
  provider,
  configured,
  hint,
  onChanged,
}: {
  provider: Provider;
  configured: boolean;
  hint: string | null;
  onChanged: () => void;
}) {
  const p = PROVIDERS[provider];
  const [editing, setEditing] = useState(false);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const showForm = !configured || editing;

  async function save() {
    setBusy(true);
    setError(null);
    try {
      await invoke<KeyStatus>("save_api_key", { provider, key });
      // La key no se conserva en el frontend una vez guardada.
      setKey("");
      setEditing(false);
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    await invoke<KeyStatus>("delete_api_key", { provider }).catch((e) => setError(String(e)));
    onChanged();
  }

  const top = (
    <>
      <div className="account-logo" aria-hidden="true">
        {p.name[0]}
      </div>
      <div className="account-info">
        <div className="account-name-line">
          <span className="account-name">{p.name}</span>
          {configured ? (
            <span className="badge ok">Configurada</span>
          ) : (
            <span className="badge">Sin configurar</span>
          )}
        </div>
        {configured && hint ? (
          <span className="account-desc">
            {DESCRIPTION[provider].short} · <span className="mono">{hint}</span>
          </span>
        ) : (
          <span className="account-desc">{DESCRIPTION[provider].long}</span>
        )}
      </div>
    </>
  );

  if (!showForm) {
    return (
      <li className="account">
        {top}
        <button type="button" className="btn btn-sm" onClick={() => setEditing(true)}>
          Cambiar
        </button>
        <button type="button" className="btn btn-sm danger" onClick={remove}>
          Borrar
        </button>
      </li>
    );
  }

  return (
    <li className="account editing">
      <div className="account-top">{top}</div>
      <div className="key-form">
        <label className="field-label">
          API key de {p.name}
          <input
            type="password"
            className="input key"
            placeholder={`${p.prefix}…`}
            autoComplete="off"
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
        </label>
        <button
          type="button"
          className="btn btn-secondary"
          disabled={busy}
          onClick={() => {
            setKey("");
            setError(null);
            setEditing(false);
          }}
        >
          Cancelar
        </button>
        <button
          type="button"
          className="btn btn-primary pad-18"
          disabled={busy || key.trim() === ""}
          onClick={save}
        >
          {busy ? "Validando…" : "Validar y guardar"}
        </button>
      </div>
      {error && <KeyErrorAlert provider={provider} error={error} />}
      <span className="account-help">
        Créala en{" "}
        <a
          href={`https://${p.site}`}
          onClick={(e) => {
            e.preventDefault();
            openSite(provider);
          }}
        >
          {p.site}
        </a>
        .
      </span>
    </li>
  );
}

export default function Accounts() {
  const [status, setStatus] = useState<KeyStatus | null>(null);
  const [hints, setHints] = useState<KeyHints>({ groq: null, anthropic: null });

  const load = useCallback(() => {
    invoke<KeyStatus>("get_key_status")
      .then(setStatus)
      .catch(() => {});
    invoke<KeyHints>("get_key_hints")
      .then((h) => h && setHints(h))
      .catch(() => {});
  }, []);

  useEffect(load, [load]);

  return (
    <main className="page">
      <div className="page-head">
        <h1 className="page-title">Cuentas</h1>
        <p className="page-lead lead-icon">
          <LockIcon size={16} />
          Las API keys se guardan en el llavero del sistema y nunca se muestran.
        </p>
      </div>
      {status && (
        <ul aria-label="API keys" className="accounts">
          {(["groq", "anthropic"] as const).map((p) => (
            <AccountCard
              key={p}
              provider={p}
              configured={status[p]}
              hint={hints[p]}
              onChanged={load}
            />
          ))}
        </ul>
      )}
    </main>
  );
}
