export const isMac = () => navigator.userAgent.includes("Mac");
/** WebKitGTK se identifica como "X11; Linux x86_64" también en Wayland. */
export const isLinux = () =>
  /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent);

/** Atajo fijo para cambiar de modo (hotkey/registry.rs). En Linux no usa Ctrl+Alt (issue #38). */
export const modeShortcut = () =>
  isMac() ? "Ctrl+Option+M" : isLinux() ? "Ctrl+Shift+M" : "Ctrl+Alt+M";

/** Atajo de dictado por defecto (hotkey/registry.rs). */
export const defaultShortcut = () =>
  isMac() ? "Option+Space" : isLinux() ? "Ctrl+Shift+Space" : "Ctrl+Alt+Space";

/** "12,8 s" */
export function seconds(ms: number): string {
  return `${(ms / 1000).toLocaleString("es", { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s`;
}
