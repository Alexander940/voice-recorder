# Inner Loop: HU-07 — API keys seguras + onboarding
> **Status:** APPROVED 2026-10-20 · ENMIENDA APPROVED 2026-10-20 (validación solo local)

**Issue:** [#7](https://github.com/Alexander940/voice-recorder/issues/7)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-07"), solo si están cerradas sus dependencias: #1, #5.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-07-keys-onboarding`.
**Objective (verifiable):** PR que cierra #7, mergeada en `main`, en la que las keys viven en `keyring`, se validan antes de guardarse, nunca llegan al frontend, y existe el asistente inicial.
**Oracle:**
- Test de ida y vuelta guardar/leer/borrar en `keyring` (con el backend simulado del crate).
- La validación usa un servidor HTTP simulado (key válida o inválida para Groq y para Anthropic).
- Un test estático que recorre los comandos `#[tauri::command]` y comprueba que ninguno devuelve un tipo que contenga la key (solo `KeyStatus`).
- Un escaneo de los logs de la suite que busca `gsk_` o `sk-ant-`: ninguna coincidencia.
- Tests de componentes (`vitest`) de cada paso del onboarding.
- **Con la API real** (`LIVE_API_TESTS=1`): validación con las keys del entorno.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-07-keys-onboarding/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); llamadas reales a Groq/Anthropic solo con `LIVE_API_TESTS=1` y las keys en variables de entorno (nunca en el repo), con un tope de 1 USD por issue sumado en el log. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
