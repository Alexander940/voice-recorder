# Inner Loop: HU-02 — App residente en la bandeja
> **Status:** APPROVED 2026-10-20 · **ENMIENDA 2026-10-20 pendiente de aprobación** (validación solo local)

**Issue:** [#2](https://github.com/Alexander940/voice-recorder/issues/2)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-02"), solo si están cerradas sus dependencias: #1.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-02-tray`.
**Objective (verifiable):** PR que cierra #2, mergeada en `main`, en la que la app arranca sin ventana principal, con un menú de bandeja (Ajustes, Historial, Modo ▸, Salir) y un icono con tres estados.
**Oracle:**
- Tests unitarios de la máquina de estados `TrayState` (Idle → Recording → Processing → Idle, cancelaciones y errores).
- Un test de que la configuración de Tauri no declara una ventana principal visible al iniciar (se parsea `tauri.conf.json`).
- Un test de que cerrar la ventana de ajustes no termina el proceso (el handler de `CloseRequested` llama a `prevent_close`).
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-02-tray/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente. Checklist manual: el icono aparece en la bandeja de Windows y cambia de estado.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); sin llamadas a APIs de pago. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.
