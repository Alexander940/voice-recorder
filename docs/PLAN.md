# Plan de implementación — Voice Recorder

App de escritorio multiplataforma (Windows, macOS, Linux) para dictar por voz desde cualquier aplicación: mantienes un atajo, hablas, y el texto transcrito y pulido aparece donde está el cursor. Inspirada en Superwhisper.

## 1. Arquitectura

```
┌───────────────────────────── Tauri 2 ─────────────────────────────┐
│  Frontend (React + TypeScript + Vite)                              │
│   • Overlay de grabación   • Ajustes   • Historial   • Onboarding  │
│                     ▲  invoke / events  │                          │
│  Backend (Rust)     │                   ▼                          │
│   hotkey ──► recorder (cpal) ──► WAV 16 kHz mono                    │
│                                     │                              │
│                                     ▼                              │
│                         stt::groq  (Whisper large-v3)              │
│                                     │ texto bruto                  │
│                                     ▼                              │
│                         llm::anthropic (Claude Haiku 5.5)          │
│                                     │ texto final                  │
│                                     ▼                              │
│                output::paste (portapapeles + Ctrl/Cmd+V)           │
│                history (SQLite)  ·  settings (store)  ·  keyring   │
└────────────────────────────────────────────────────────────────────┘
```

**Por qué dos proveedores:** la API de Claude no acepta audio (solo texto, imágenes y documentos). La transcripción la hace **Groq Whisper** y la limpieza y los modos los hace **Claude**.

## 2. Stack

| Capa | Elección | Notas |
|---|---|---|
| Shell de escritorio | Tauri 2 | Binario ligero, multiplataforma |
| Frontend | React + TypeScript + Vite | UI de ajustes, historial y overlay |
| Atajo global | `tauri-plugin-global-shortcut` | Alternar (por defecto) y mantener pulsado |
| Bandeja | Tray API de Tauri 2 | La app vive en la bandeja |
| Captura de audio | `cpal` + `hound` | Remuestreo a 16 kHz mono WAV |
| Voz a texto | Groq API `whisper-large-v3(-turbo)` | `POST /openai/v1/audio/transcriptions` (multipart) vía `reqwest` |
| LLM | Claude Haiku 5.5 (`claude-haiku-5-5`) | HTTP directo a `POST /v1/messages` desde Rust (no hay SDK oficial de Rust); `output_config.effort: "low"` para minimizar la latencia |
| Pegado | `tauri-plugin-clipboard-manager` + `enigo` | Guardar el portapapeles, pegar y restaurarlo |
| Secretos | `keyring` | API keys en el almacén seguro del SO, nunca en texto plano |
| Ajustes | `tauri-plugin-store` | JSON de configuración |
| Historial | SQLite (`rusqlite` o `tauri-plugin-sql`) | Transcripciones y metadatos |
| CI/CD | GitHub Actions + `tauri-action` | Builds de Win/macOS/Linux |

## 3. Requisitos previos de desarrollo

- Rust (rustup, toolchain estable) — **no está instalado todavía en esta máquina**
- Node 22 y npm (ya instalados)
- Windows: Microsoft C++ Build Tools y WebView2
- macOS: Xcode CLT. Linux: `webkit2gtk`, `libayatana-appindicator`, `libasound2-dev`
- Cuentas: API key de Groq y API key de Anthropic

## 4. Fases y entregables

| Fase | Objetivo | Historias |
|---|---|---|
| **F0 — Fundaciones** | Proyecto compilando en las 3 plataformas con CI | #1 |
| **F1 — MVP de dictado** | Atajo → grabar → transcribir → pegar | #2, #3, #4, #5, #6, #7 |
| **F2 — Inteligencia con Claude** | Limpieza, modos y vocabulario | #8, #9, #10 |
| **F3 — Experiencia de usuario** | Historial, ajustes y robustez | #11, #12, #13 |
| **F4 — Distribución** | Instaladores firmados y actualizaciones automáticas | #14, #15 |

## 5. Flujo principal (F1 + F2)

1. El usuario mantiene el atajo (por defecto `Ctrl+Alt+Space` / `Cmd+Option+Space`).
2. Se muestra el overlay de grabación y `cpal` captura el micrófono por defecto.
3. Al soltar el atajo, se codifica a WAV de 16 kHz mono en memoria.
4. Se envía a Groq Whisper con el idioma (o detección automática) y el vocabulario como `prompt`.
5. Si el modo activo tiene limpieza, se envía el texto a Claude Haiku 5.5 con el prompt del modo.
6. Se pega el resultado en la app activa y se guarda en el historial.

**Objetivo de latencia:** menos de 1,5 s desde que se suelta la tecla hasta el pegado, para dictados de hasta 30 s.

## 6. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| Pegar en algunas apps falla (procesos elevados, terminales) | Recurso alternativo: dejar el texto en el portapapeles y avisar con una notificación |
| macOS exige permisos de micrófono y de accesibilidad | Pantalla de onboarding que guíe al usuario a concederlos |
| La latencia suma STT y LLM | Haiku con esfuerzo bajo, un timeout y la opción "sin limpieza" |
| El antivirus detecta la simulación de pulsaciones | Firmar el código (F4) |
| Las API keys pueden filtrarse | Usar `keyring` y no registrar nunca las keys en los logs |
| El atajo puede chocar con otro de la app o del SO | Atajo configurable y detección de conflictos |

## 7. Costes de API

- **Transcripción:** Groq Whisper cobra por hora de audio (consultar la tarifa vigente de Groq).
- **Limpieza:** Claude Haiku 5.5 cuesta $0.10 por millón de tokens de entrada y $0.50 por millón de salida. Un dictado típico (~200 tokens de entrada y otros tantos de salida) cuesta una fracción de centavo.

## 8. Definición de terminado

Para cada historia: criterios de aceptación cumplidos, probada en Windows (y en macOS/Linux si afecta a código específico de plataforma), CI en verde y sin secretos en el repositorio.
