# Voice Recorder

App de escritorio multiplataforma (Tauri 2) para dictar por voz desde cualquier aplicación. Pulsas un atajo, hablas, lo vuelves a pulsar y el texto transcrito (Groq Whisper) y pulido por Claude Haiku 5.5 se pega donde está el cursor.

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

## Publicar una versión

1. **Claves del updater** (ya hechas para v0.1.0; solo hay que repetirlas si se pierden):
   - `npx tauri signer generate -w ~/.tauri/voice-recorder.key` (en PowerShell: `npx.cmd`).
   - La clave pública está en `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
   - La clave privada y su contraseña están en los secretos `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` de GitHub. Nunca van al repo. Guarda una copia: sin ella, las instalaciones existentes no pueden aceptar nuevas versiones.
   - Los artefactos firmados del updater solo se generan en el workflow (`src-tauri/tauri.release.conf.json`). `npm run tauri build` en local no necesita la clave.
2. (Opcional) Secretos de firma de código: `APPLE_*` para macOS y `WINDOWS_CERTIFICATE` / `WINDOWS_CERTIFICATE_PASSWORD` para Windows.
3. Ensayo sin publicar: _Actions → Release → Run workflow_.
4. Release: sube la versión en `tauri.conf.json` y `package.json`, añade la entrada en `CHANGELOG.md`, luego `git tag vX.Y.Z && git push origin vX.Y.Z` → revisa el borrador en GitHub Releases (instaladores y `latest.json`) → publícalo.

El updater busca `releases/latest/download/latest.json`: solo las releases **publicadas** (no los borradores) llegan a los usuarios.
