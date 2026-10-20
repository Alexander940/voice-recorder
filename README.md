# Voice Recorder

App de escritorio multiplataforma (Tauri 2) para dictar por voz desde cualquier aplicación. Mantienes un atajo, hablas, y el texto transcrito (Groq Whisper) y pulido por Claude Haiku 5.5 se pega donde está el cursor.

- Plan de implementación: [docs/PLAN.md](docs/PLAN.md)
- Historias de usuario: [issues](../../issues)
- Loops de implementación: [docs/loops-history](docs/loops-history/README.md)

## Requisitos previos

| Plataforma | Necesitas                                                                                                                                                   |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Todas      | [Rust](https://rustup.rs) (stable) y Node 22+                                                                                                               |
| Windows    | Visual Studio Build Tools con la carga de trabajo **"Desarrollo para el escritorio con C++"** (MSVC y Windows 10/11 SDK), y WebView2 (viene con Windows 11) |
| macOS      | Xcode Command Line Tools (`xcode-select --install`)                                                                                                         |
| Linux      | `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libxdo-dev libdbus-1-dev patchelf`                                          |

Para las funciones de IA hacen falta una API key de [Groq](https://console.groq.com) y otra de [Anthropic](https://console.anthropic.com).

## Desarrollo

```bash
npm install
npm run tauri dev      # app en modo desarrollo
npm run lint           # eslint + prettier
npm run typecheck      # tsc
npm test               # vitest
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Estructura

```
src/                 Frontend React + TypeScript
src-tauri/src/
  audio/             Captura de micrófono → WAV 16 kHz mono
  stt/               Voz a texto (Groq Whisper)
  llm/               Limpieza y modos (Claude)
  output/            Portapapeles y pegado en la app activa
  settings/          Configuración y API keys (keyring)
  history/           Historial en SQLite
```
