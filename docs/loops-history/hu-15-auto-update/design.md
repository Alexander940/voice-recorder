# Inner Loop: HU-15 — Actualizaciones automáticas
> **Status:** APPROVED 2026-10-20 · ENMIENDA APPROVED 2026-10-20 (validación solo local) · ENMIENDA 2 APPROVED 2026-10-21

**Issue:** [#15](https://github.com/Alexander940/voice-recorder/issues/15)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-15"), solo si están cerradas sus dependencias: #14.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-15-auto-update`.
**Objective (verifiable):** PR que cierra #15, mergeada en `main`, en la que `tauri-plugin-updater` está configurado con endpoint en GitHub Releases y firma, hace una comprobación al inicio y una diaria, pide confirmación y se puede desactivar.
**Oracle:**
- Test de integración con un manifiesto servido en local (`wiremock`): con una versión mayor ofrece la actualización; con una firma inválida la rechaza; desactivada, no comprueba.
- Test del programador de comprobaciones (como máximo una al día, con un reloj simulado).
- La clave privada del updater la genera el usuario y la guarda en GitHub Secrets; el agente nunca la ve.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-15-auto-update/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); generar o guardar la clave de firma del updater (lo hace el usuario). Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.

## Enmienda 2 (2026-10-21): sin release real
Como HU-14 no publica releases, este loop **no** valida contra GitHub Releases:
- El endpoint del updater apunta a `https://github.com/Alexander940/voice-recorder/releases/latest/download/latest.json`, pero la validación usa **solo un manifiesto local** (mockito): versión mayor → ofrece la actualización; firma inválida → la rechaza; desactivado → no comprueba; como mucho una comprobación al día (reloj simulado).
- La verificación de firma se prueba con un **par de claves de prueba** generado en un directorio temporal durante el test. La **clave pública real** para `tauri.conf.json` la genera el usuario con `npx tauri signer generate` (el agente nunca ve la privada); mientras no exista, el updater queda desactivado en la configuración y la app funciona igual.
- No se valida (riesgo aceptado): una actualización real de extremo a extremo entre dos releases publicadas.
