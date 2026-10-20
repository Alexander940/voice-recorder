# Inner Loop: HU-12 — Ventana de ajustes
> **Status:** APPROVED 2026-10-20 · **ENMIENDA 2026-10-20 pendiente de aprobación** (validación solo local)

**Issue:** [#12](https://github.com/Alexander940/voice-recorder/issues/12)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-12"), solo si están cerradas sus dependencias: #3, #4, #7, #9.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-12-settings`.
**Objective (verifiable):** PR que cierra #12, mergeada en `main`, en la que la ventana tiene todas sus secciones, los cambios persisten y se aplican sin reiniciar, y existe el inicio con el sistema.
**Oracle:**
- Tests de que la configuración serializa y deserializa con valores por defecto y de que un archivo antiguo migra.
- Test de que un cambio de ajuste emite un evento y el backend lo aplica (por ejemplo, el atajo se vuelve a registrar), con el plugin simulado.
- Tests de componentes (`vitest`) de cada sección.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-12-settings/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); sin llamadas a APIs de pago. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
