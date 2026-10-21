export const isMac = () => navigator.userAgent.includes("Mac");

/** Atajo fijo para cambiar de modo (hotkey/registry.rs). */
export const modeShortcut = () => (isMac() ? "Ctrl+Option+M" : "Ctrl+Alt+M");

/** Atajo de dictado por defecto (hotkey/registry.rs). */
export const defaultShortcut = () => (isMac() ? "Option+Space" : "Ctrl+Alt+Space");

/** "12,8 s" */
export function seconds(ms: number): string {
  return `${(ms / 1000).toLocaleString("es", { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s`;
}
