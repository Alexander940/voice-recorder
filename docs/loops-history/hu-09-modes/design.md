# Inner Loop: HU-09 — Modos personalizados
> **Status:** APPROVED 2026-10-20 · ENMIENDA APPROVED 2026-10-20 (validación solo local)

**Issue:** [#9](https://github.com/Alexander940/voice-recorder/issues/9)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-09"), solo si están cerradas sus dependencias: #8.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-09-modes`.
**Objective (verifiable):** PR que cierra #9, mergeada en `main`, en la que existen los 6 modos predefinidos, se pueden crear, editar, duplicar y borrar, se cambian desde la bandeja y con un atajo, y el overlay muestra el modo.
**Oracle:**
- Tests de persistencia de los modos (store simulado), del armado del prompt por modo y de que el modo Literal no llama al LLM.
- Tests de componentes (`vitest`) del editor de modos.
- **Evaluación con la API real**: Traducir (ES → EN) da inglés según `whatlang`; Email contiene un saludo y una despedida (regex); Prompt técnico no lleva relleno conversacional. Al menos 5 de 6 casos.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-09-modes/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); llamadas reales a Groq/Anthropic solo con `LIVE_API_TESTS=1` y las keys en variables de entorno (nunca en el repo), con un tope de 1 USD por issue sumado en el log. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.

## Outcome
**Resultado:** ✅ éxito, en 2 iteraciones de validación local más la evaluación en vivo (2026-10-21).

| Ítem | Veredicto | Evidencia |
|---|---|---|
| Validación local, iteración 1 | fail | error de compilación: faltaba re-exportar `mode_shortcut` |
| Validación local, iteración 2 | pass | cargo test 109/109, clippy, lint, tsc, vitest 25/25 y arranque |
| Evaluación en vivo | pass | 6/6 modos; < $0,0005 |
| Entrega | merged | PR #25 → `0844d41`, #9 cerrada |
