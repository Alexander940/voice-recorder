import type { ReactNode } from "react";
import { BookIcon, ClockIcon, KeyIcon, LayersIcon, RefreshIcon, SlidersIcon } from "./icons";

export type Section = "history" | "modes" | "vocabulary" | "settings" | "accounts" | "updates";
export type TrayState = "idle" | "recording" | "processing";

const STATUS: Record<TrayState, string> = {
  idle: "Listo para dictar",
  recording: "Grabando…",
  processing: "Procesando…",
};

const PRIMARY: [Section, string, () => ReactNode][] = [
  ["history", "Historial", () => <ClockIcon />],
  ["modes", "Modos", () => <LayersIcon />],
  ["vocabulary", "Vocabulario", () => <BookIcon />],
];
const SECONDARY: [Section, string, () => ReactNode][] = [
  ["settings", "Ajustes", () => <SlidersIcon />],
  ["accounts", "Cuentas", () => <KeyIcon />],
  ["updates", "Actualizaciones", () => <RefreshIcon />],
];

interface Props {
  section: Section;
  onNavigate: (s: Section) => void;
  state: TrayState;
  shortcut: string;
  modeName: string;
  children: ReactNode;
}

export default function Shell({ section, onNavigate, state, shortcut, modeName, children }: Props) {
  const item = ([id, label, iconEl]: [Section, string, () => ReactNode]) => (
    <button
      key={id}
      type="button"
      className="nav-item"
      aria-current={section === id ? "page" : undefined}
      onClick={() => onNavigate(id)}
    >
      {iconEl()}
      {label}
    </button>
  );
  return (
    <div className="window">
      <nav className="nav" aria-label="Secciones">
        {PRIMARY.map(item)}
        <div className="nav-sep" />
        {SECONDARY.map(item)}
        <div className="grow" />
        <div className="status-card" role="status" aria-live="polite">
          <div className="status-line">
            <span className="status-dot" data-state={state} />
            {STATUS[state]}
          </div>
          <span className="kbd">{shortcut}</span>
          <span className="status-mode">Modo: {modeName}</span>
        </div>
      </nav>
      {children}
    </div>
  );
}
