# Inner Loop: HU-06 — Pegar el texto en la app activa
> **Status:** APPROVED 2026-10-20

**Issue:** [#6](https://github.com/Alexander940/voice-recorder/issues/6)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-06"), solo si están cerradas sus dependencias: #1.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-06-paste-output`.
**Objective (verifiable):** PR que cierra #6, mergeada en `main`, en la que el pegado guarda el portapapeles, escribe el texto, simula pegar, restaura el portapapeles y, si falla, deja el texto en el portapapeles y notifica; existe la opción de solo copiar.
**Oracle:**
- Los traits `Clipboard` y `KeySender` son simulados. Los tests verifican el orden exacto de las llamadas (get → set → paste → espera → restore), el recurso alternativo cuando `paste` falla (sin restaurar y con notificación) y el modo de solo copiar.
- Un test de que el atajo de pegar es Cmd en macOS y Ctrl en el resto (`cfg`).
- Un test de integración en Windows (`#[cfg(windows)]`, `#[ignore]` en la CI): portapapeles real, ida y vuelta.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` y la matriz de CI (Windows/macOS/Linux) en verde. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local **y** la CI de la PR está en verde; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-06-paste-output/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación y la CI pasan (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente. Checklist manual: pegar en el navegador, VS Code, Word, Slack y el Bloc de notas.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación y la CI pasan); sin llamadas a APIs de pago. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
