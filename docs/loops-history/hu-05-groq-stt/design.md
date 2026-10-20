# Inner Loop: HU-05 — Transcripción con Groq Whisper
> **Status:** APPROVED 2026-10-20

**Issue:** [#5](https://github.com/Alexander940/voice-recorder/issues/5)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-05"), solo si están cerradas sus dependencias: #4.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-05-groq-stt`.
**Objective (verifiable):** PR que cierra #5, mergeada en `main`, en la que el trait `Transcriber` tiene una implementación `GroqTranscriber` con modelo e idioma configurables, timeout, un reintento y errores tipados.
**Oracle:**
- Servidor HTTP simulado (`wiremock`): el cuerpo multipart contiene `file`, `model` y `language`; un 5xx provoca un único reintento; un 401 devuelve `AuthError` y un 429 `RateLimited`; el timeout se respeta.
- Un test de que ningún log contiene la API key (se captura con `tracing-test`).
- **Con la API real** (`LIVE_API_TESTS=1`): `fixtures/es_sample.wav` con la transcripción de referencia, y el WER debe ser como máximo 0,15.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` y la matriz de CI (Windows/macOS/Linux) en verde. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local **y** la CI de la PR está en verde; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-05-groq-stt/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación y la CI pasan (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación y la CI pasan); llamadas reales a Groq/Anthropic solo con `LIVE_API_TESTS=1` y las keys en variables de entorno (nunca en el repo), con un tope de 1 USD por issue sumado en el log. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
