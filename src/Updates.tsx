import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Available {
  version: string;
  notes: string | null;
}

export interface UpdateStatus {
  configured: boolean;
  enabled: boolean;
  available: Available | null;
}

interface Props {
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
}

export default function Updates({ enabled, onToggle }: Props) {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
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
    <section aria-labelledby="s-updates">
      <h3 id="s-updates">Actualizaciones</h3>
      <label>
        <input type="checkbox" checked={enabled} onChange={(e) => onToggle(e.target.checked)} />
        Buscar actualizaciones al iniciar y una vez al día
      </label>
      {status && !status.configured && (
        <p>Las actualizaciones automáticas aún no están configuradas en esta compilación.</p>
      )}
      {status?.available && (
        <div role="status">
          Hay una nueva versión: <strong>{status.available.version}</strong>
          {status.available.notes && <p>{status.available.notes}</p>}
          <button onClick={install} disabled={installing}>
            {installing ? "Instalando…" : "Instalar y reiniciar"}
          </button>
        </div>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
