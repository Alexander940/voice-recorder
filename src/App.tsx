import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import History from "./History";
import Settings from "./Settings";
import Onboarding, { type KeyStatus } from "./Onboarding";

export type View = "settings" | "history" | "onboarding";

const TITLES: Record<Exclude<View, "onboarding">, string> = {
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
    // El evento `navigate` del primer arranque puede llegar antes que este
    // listener; el estado de las keys decide si hace falta el onboarding.
    invoke<KeyStatus>("get_key_status")
      .then((s) => {
        if (!s.groq) setView("onboarding");
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    // El menú de la bandeja indica qué vista abrir.
    const unlisten = listen<View>("navigate", (e) => setView(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (view === "onboarding") {
    return (
      <main>
        <Onboarding onDone={() => setView("settings")} />
      </main>
    );
  }

  return (
    <main>
      <h1>Voice Recorder</h1>
      <h2>{TITLES[view]}</h2>
      {view === "history" ? <History /> : <Settings />}
      {version && <p data-testid="version">v{version}</p>}
    </main>
  );
}
