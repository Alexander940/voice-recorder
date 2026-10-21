import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Overlay from "./Overlay";
import "./styles/app.css";

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
const harness = new URLSearchParams(window.location.search).get("harness");
// La ventana del overlay carga `index.html#overlay`.
const isOverlay = harness ? harness.startsWith("overlay") : window.location.hash === "#overlay";
document.documentElement.dataset.window = isOverlay ? "overlay" : "main";

if (import.meta.env.DEV && harness) {
  // Arnés del oráculo visual: solo existe en desarrollo y Vite lo elimina del build.
  import("./harness").then(({ setupHarness, finishHarness }) => {
    const screen = setupHarness(harness);
    root.render(<React.StrictMode>{screen.render()}</React.StrictMode>);
    finishHarness(screen);
  });
} else {
  const Root = isOverlay ? Overlay : App;
  root.render(
    <React.StrictMode>
      <Root />
    </React.StrictMode>,
  );
}
