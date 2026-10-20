# Inner Loop: HU-14 — Instaladores firmados
> **Status:** APPROVED 2026-10-20 · ENMIENDA APPROVED 2026-10-20 (validación solo local) · **ENMIENDA 2 (2026-10-21) pendiente de aprobación**

**Issue:** [#14](https://github.com/Alexander940/voice-recorder/issues/14)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-14"), solo si están cerradas sus dependencias: #1 y el MVP (#2–#7).
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-14-installers`.
**Objective (verifiable):** PR que cierra #14, mergeada en `main`, en la que el workflow `release.yml` genera msi/exe, dmg y AppImage/deb a partir de un tag `v*`, con firma configurable por secretos y `NSMicrophoneUsageDescription`.
**Oracle:**
- `actionlint` sobre los workflows.
- Un `workflow_dispatch` de ensayo (sin publicar) que produce los artefactos esperados (la lista se comprueba con un script).
- Test de que `tauri.conf.json`/`Info.plist` contienen las descripciones de permisos.
- **Enmienda 2026-10-20:** este loop necesita GitHub Actions por naturaleza (builds de macOS/Linux); al llegar aquí se decidirá con el usuario cómo reactivarla.
- La firma se valida solo cuando el usuario carga los secretos; mientras tanto, los artefactos sin firmar son el criterio de éxito.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-14-installers/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente. Paso adicional: el usuario decide cuándo se publica la primera release.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); los certificados de firma (que tienen coste) los compra y carga el usuario; el **tag y la release reales los crea el usuario**. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.

## Enmienda 2 (2026-10-21): Windows en local y workflow sin ejecutar
Decisión del usuario: no gastar minutos de Actions. **Sustituye** a la validación anterior de este loop:

**Objective (verifiable):** PR que cierra #14, mergeada, en la que (a) \`release.yml\` define los builds de Windows (msi/nsis), macOS universal (dmg) y Linux (AppImage/deb) a partir de un tag \`v*\` o de \`workflow_dispatch\` (ensayo sin publicar), con firma opcional mediante secretos; (b) el instalador de Windows se genera en local; (c) los permisos de macOS están declarados.

**Oracle (solo local):**
- \`actionlint\` (binario oficial de rhysd/actionlint descargado en el scratchpad) sin errores sobre \`.github/workflows/*.yml\`.
- \`npm run tauri build\` en Windows termina con código 0 y \`scripts/check-artifacts.mjs\` confirma un \`.msi\` y un \`-setup.exe\` con el nombre y la versión esperados.
- Test de Rust: \`Info.plist\` contiene \`NSMicrophoneUsageDescription\` (y el texto de accesibilidad) y \`tauri.conf.json\` lo referencia; \`bundle.targets\` incluye msi, nsis, dmg, appimage y deb.
- Común: fmt, clippy, cargo test, lint, tsc y vitest.

**No se valida (riesgo aceptado):** que macOS y Linux compilen, la firma real y la publicación. Quedan para cuando el usuario lance el workflow a mano.
**Irreversible actions:** ninguna nueva. **No** se crean tags ni releases ni se lanza el workflow; los certificados y secretos los gestiona el usuario.
