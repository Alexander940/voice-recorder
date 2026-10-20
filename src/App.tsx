import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type View = "settings" | "history";

const TITLES: Record<View, string> = {
  settings: "Ajustes",
  history: "Historial",
};

export default function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [view, setView] = useState<View>("settings");

  useEffect(() => {
    invoke<string>("app_version")
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  useEffect(() => {
    // El menú de la bandeja indica qué vista abrir.
    const unlisten = listen<View>("navigate", (e) => setView(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <main>
      <h1>Voice Recorder</h1>
      <h2>{TITLES[view]}</h2>
      <p>En construcción.</p>
      {version && <p data-testid="version">v{version}</p>}
    </main>
  );
}
