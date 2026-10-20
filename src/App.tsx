import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function App() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("app_version")
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  return (
    <main>
      <h1>Voice Recorder</h1>
      <p>En construcción.</p>
      {version && <p data-testid="version">v{version}</p>}
    </main>
  );
}
