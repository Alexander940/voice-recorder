# Inner Loop: HU-11 — Historial de transcripciones
> **Status:** APPROVED 2026-10-20

**Issue:** [#11](https://github.com/Alexander940/voice-recorder/issues/11)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-11"), solo si están cerradas sus dependencias: #5.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-11-history`.
**Objective (verifiable):** PR que cierra #11, mergeada en `main`, en la que el historial en SQLite permite buscar, copiar, repegar y borrar, tiene retención configurable y un modo sin historial.
**Oracle:**
- Tests del repositorio con SQLite en memoria: insertar y leer, búsqueda de texto, purga por retención con una fecha fija, el modo sin historial no escribe, y el audio no se guarda por defecto.
- Test de migraciones: una base vacía queda en el esquema actual.
- Tests de componentes (`vitest`) de la lista y la búsqueda.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` y la matriz de CI (Windows/macOS/Linux) en verde. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local **y** la CI de la PR está en verde; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-11-history/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación y la CI pasan (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación y la CI pasan); sin llamadas a APIs de pago. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
