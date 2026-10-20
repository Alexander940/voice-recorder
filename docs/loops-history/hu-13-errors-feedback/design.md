# Inner Loop: HU-13 — Errores y feedback
> **Status:** APPROVED 2026-10-20 · ENMIENDA APPROVED 2026-10-20 (validación solo local)

**Issue:** [#13](https://github.com/Alexander940/voice-recorder/issues/13)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-13"), solo si están cerradas sus dependencias: #5, #8, #11.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-13-errors-feedback`.
**Objective (verifiable):** PR que cierra #13, mergeada en `main`, en la que cada error tipado tiene su notificación, hay sonidos opcionales, un reintento desde el historial y logs rotativos sin datos sensibles.
**Oracle:**
- Una tabla de tests que relaciona cada variante de `AppError` con su notificación (título y texto en ES/EN).
- Test del filtro de logs: las keys, el texto dictado y las rutas de usuario aparecen ocultas.
- Test de reintento: si la transcripción falla, el audio queda en memoria; al reintentar desde el historial se usa un transcriptor simulado y se completa.
- Test de rotación de logs (el tamaño máximo se respeta).
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-13-errors-feedback/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); sin llamadas a APIs de pago. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.

## Outcome
**Resultado:** ✅ éxito, en 1 iteración de validación local (2026-10-21).

| Ítem | Veredicto | Evidencia |
|---|---|---|
| Validación local, iteración 1 | pass | cargo test 140/140, clippy, lint, tsc, vitest 39/39 y arranque con app.log sin keys |
| Entrega | merged | PR #28 → `a0c4f48`, #13 cerrada |
