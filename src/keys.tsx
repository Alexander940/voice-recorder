import { invoke } from "@tauri-apps/api/core";
import { AlertIcon } from "./icons";

export type Provider = "groq" | "anthropic";

export interface KeyStatus {
  groq: boolean;
  anthropic: boolean;
}

export interface KeyHints {
  groq: string | null;
  anthropic: string | null;
}

export const PROVIDERS: Record<
  Provider,
  { name: string; prefix: string; site: string; xai?: boolean }
> = {
  groq: { name: "Groq", prefix: "gsk_", site: "console.groq.com", xai: true },
  anthropic: { name: "Anthropic", prefix: "sk-ant-", site: "console.anthropic.com" },
};

/** Abre la consola del proveedor en el navegador (destinos fijos en el backend). */
export function openSite(target: Provider | "accessibility") {
  invoke("open_link", { target }).catch(() => {});
}

/** El backend marca así las keys con formato incorrecto (KeyError::WrongFormat). */
const WRONG_FORMAT = "No parece una key de";

export function KeyErrorAlert({ provider, error }: { provider: Provider; error: string }) {
  const p = PROVIDERS[provider];
  return (
    <div role="alert" className="alert alert-danger">
      <AlertIcon stroke="var(--danger)" />
      {error.startsWith(WRONG_FORMAT) ? (
        <span>
          Esta key no parece de {p.name}: debe empezar por{" "}
          <strong className="mono">{p.prefix}</strong>.
          {p.xai ? " Las keys de xAI (Grok) no sirven." : ""}
        </span>
      ) : (
        <span>{error}</span>
      )}
    </div>
  );
}
