# Inner Loop: HU-04 — Captura de audio + overlay
> **Status:** APPROVED 2026-10-20

**Issue:** [#4](https://github.com/Alexander940/voice-recorder/issues/4)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-04"), solo si están cerradas sus dependencias: #1, #3.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-04-audio-capture`.
**Objective (verifiable):** PR que cierra #4, mergeada en `main`, en la que la captura produce un WAV PCM de 16 kHz mono en memoria, descarta lo que dura menos de 300 ms, aplica el límite configurable y el overlay muestra el nivel y el tiempo.
**Oracle:**
- `cpal` va detrás del trait `AudioSource`. Los tests usan un `FileAudioSource` con fixtures de 44.1 kHz estéreo y 48 kHz mono, y el WAV leído de nuevo con `hound` debe ser de 16 kHz, mono y 16 bits, con una duración de ±1 %.
- Tests de que una grabación de 200 ms se descarta y de que se corta al llegar al límite.
- Un test del cálculo de nivel RMS con valores conocidos.
- Un test de componente (`vitest`) del overlay que recibe eventos de nivel.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` y la matriz de CI (Windows/macOS/Linux) en verde. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local **y** la CI de la PR está en verde; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-04-audio-capture/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación y la CI pasan (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente. Checklist manual: grabar con el micrófono real y oír el WAV de depuración.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación y la CI pasan); sin llamadas a APIs de pago. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
