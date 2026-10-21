import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Accounts from "./Accounts";
import History from "./History";
import Modes, { type ModesState } from "./Modes";
import Onboarding, { type KeyStatus } from "./Onboarding";
import Settings, { type AppSettings } from "./Settings";
import Shell, { type Section, type TrayState } from "./Shell";
import Updates from "./Updates";
import Vocabulary from "./Vocabulary";
import { defaultShortcut } from "./platform";

export type View = Section | "onboarding";

const SECTIONS: View[] = [
  "history",
  "modes",
  "vocabulary",
  "settings",
  "accounts",
  "updates",
  "onboarding",
];

export default function App({ initialView = "history" }: { initialView?: View }) {
  const [view, setView] = useState<View>(initialView);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [modes, setModes] = useState<ModesState | null>(null);
  const [tray, setTray] = useState<TrayState>("idle");

  const loadModes = useCallback(() => {
    invoke<ModesState>("modes_list")
      .then(setModes)
      .catch(() => {});
  }, []);

  useEffect(() => {
    invoke<AppSettings>("settings_get")
      .then(setSettings)
      .catch(() => {});
    loadModes();
    // El evento `navigate` del primer arranque puede llegar antes que este
    // listener; el estado de las keys decide si hace falta el onboarding.
    invoke<KeyStatus>("get_key_status")
      .then((s) => {
        if (s && !s.groq) setView("onboarding");
      })
      .catch(() => {});
  }, [loadModes]);

  useEffect(() => {
    const subs = [
      // El menú de la bandeja indica qué vista abrir.
      listen<View>("navigate", (e) => {
        if (SECTIONS.includes(e.payload)) setView(e.payload);
      }),
      listen<AppSettings>("settings-changed", (e) => setSettings(e.payload)),
      listen<string>("mode-changed", () => loadModes()),
      listen<TrayState>("tray-state", (e) => setTray(e.payload)),
    ];
    return () => {
      subs.forEach((s) => s.then((fn) => fn()));
    };
  }, [loadModes]);

  if (view === "onboarding") {
    return <Onboarding onDone={() => setView("history")} />;
  }

  const modeName = modes?.modes.find((m) => m.id === modes.active)?.name ?? "";
  const shortcut = settings?.hotkey.shortcut ?? defaultShortcut();

  return (
    <Shell section={view} onNavigate={setView} state={tray} shortcut={shortcut} modeName={modeName}>
      {view === "history" && <History modes={modes} />}
      {view === "modes" && <Modes state={modes} onChange={setModes} />}
      {view === "vocabulary" && <Vocabulary />}
      {view === "settings" && <Settings onSaved={setSettings} />}
      {view === "accounts" && <Accounts />}
      {view === "updates" && <Updates />}
    </Shell>
  );
}
