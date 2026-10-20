# Inner Loop: HU-08 — Limpieza con Claude Haiku 5.5
> **Status:** APPROVED 2026-10-20 · ENMIENDA APPROVED 2026-10-20 (validación solo local)

**Issue:** [#8](https://github.com/Alexander940/voice-recorder/issues/8)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-08"), solo si están cerradas sus dependencias: #5, #7.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-08-claude-cleanup`.
**Objective (verifiable):** PR que cierra #8, mergeada en `main`, en la que la limpieza llama a `/v1/messages` con `claude-haiku-5-5` y `effort: low`, tiene un recurso alternativo al texto bruto ante error, timeout o `refusal`, y se puede desactivar.
**Oracle:**
- Servidor HTTP simulado: las cabeceras `x-api-key` y `anthropic-version`, `model == claude-haiku-5-5` y `output_config.effort == low`; las respuestas con `stop_reason` `refusal` o `max_tokens` y un timeout de 3 s devuelven el texto bruto.
- **Evaluación con la API real** (`LIVE_API_TESTS=1`): `fixtures/cleanup_cases.json` con unos 10 casos ES/EN y comprobaciones **deterministas**: no empieza con un preámbulo (regex), no contiene las muletillas listadas, mantiene el idioma (crate `whatlang`), conserva las palabras clave del caso, la longitud está entre el 50 % y el 130 % de la entrada, y no responde a preguntas dictadas. Debe pasar en al menos 9 de 10 casos.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-08-claude-cleanup/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); llamadas reales a Groq/Anthropic solo con `LIVE_API_TESTS=1` y las keys en variables de entorno (nunca en el repo), con un tope de 1 USD por issue sumado en el log. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
