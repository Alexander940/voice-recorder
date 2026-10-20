# Inner Loop: HU-01 — Scaffold Tauri 2 + CI multiplataforma
> **Status:** APPROVED 2026-10-20

**Issue:** [#1](https://github.com/Alexander940/voice-recorder/issues/1)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-01"), solo si están cerradas sus dependencias: ninguna.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-01-scaffold-ci`.
**Objective (verifiable):** PR que cierra #1, mergeada en `main`, en la que `npm run tauri dev` arranca, existen los módulos Rust `audio`, `stt`, `llm`, `output`, `settings` y `history`, y el workflow de CI compila en los 3 sistemas.
**Oracle:**
- `npm run tauri build` termina con código 0 en Windows (local).
- El workflow `ci.yml` se ejecuta en la PR con los jobs `windows-latest`, `macos-latest` y `ubuntu-latest` en verde.
- Un script `scripts/check-structure` verifica que existen los módulos y los scripts npm requeridos.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` y la matriz de CI (Windows/macOS/Linux) en verde. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local **y** la CI de la PR está en verde; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-01-scaffold-ci/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación y la CI pasan (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación y la CI pasan); sin llamadas a APIs de pago. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
