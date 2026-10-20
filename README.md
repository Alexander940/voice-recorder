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

## Publicar una versión

1. **Claves del updater** (una sola vez): `npx tauri signer generate -w ~/.tauri/voice-recorder.key`
   - Copia la **clave pública** en `src-tauri/tauri.conf.json` → `plugins.updater.pubkey` y pon `bundle.createUpdaterArtifacts: true`
   - Guarda la **clave privada** y su contraseña como secretos `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` en GitHub. No la subas al repo
2. (Opcional) Secretos de firma de código: `APPLE_*` para macOS y `WINDOWS_CERTIFICATE` / `WINDOWS_CERTIFICATE_PASSWORD` para Windows
3. Ensayo sin publicar: _Actions → Release → Run workflow_
4. Release: sube la versión en `tauri.conf.json`, luego `git tag vX.Y.Z && git push origin vX.Y.Z` → revisa el borrador en GitHub Releases → publícalo

Sin clave pública, la app no busca actualizaciones y funciona igual.
