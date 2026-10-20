# Inner Loop: HU-10 — Vocabulario personalizado
> **Status:** APPROVED 2026-10-20 · ENMIENDA APPROVED 2026-10-20 (validación solo local)

**Issue:** [#10](https://github.com/Alexander940/voice-recorder/issues/10)
**Classification:** inner loop (tarea cerrada con validación ejecutable)
**Trigger:** orden del usuario ("ejecuta el loop HU-10"), solo si están cerradas sus dependencias: #5, #8.
**Executor:** agente en el chat (Claude Code en esta sesión o en una que la retome). Una issue a la vez, en la rama `feat/hu-10-vocabulary`.
**Objective (verifiable):** PR que cierra #10, mergeada en `main`, en la que el vocabulario se envía a Whisper (recortado al límite) y a Claude, y se aplican los reemplazos.
**Oracle:**
- Test de que el prompt de Whisper se recorta a 224 tokens como máximo (aproximado) sin cortar términos.
- Tests de los reemplazos (sin distinguir mayúsculas, solo palabras completas, sin reemplazar dentro de otras palabras).
- **Con la API real**: `fixtures/vocab_sample.wav` con 3 términos propios, que aparecen con su ortografía exacta en al menos 2 de 3.
- Común: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm run lint`, `npm run typecheck`, `npm test` en local (Windows). **Sin CI** (enmienda 2026-10-20: el usuario la desactivó por consumo de minutos). macOS y Linux no se validan hasta que se reactive. El agente nunca da por buena su propia salida sin estas comprobaciones.
**Exit criteria:** éxito = la validación pasa en local; fallo = 5 iteraciones fallidas **o** 2 h de reloj **o** 1 USD de gasto en APIs reales → la PR pasa a draft, se comenta en la issue el diagnóstico y el log, y se escala al usuario.
**Memory:** `docs/loops-history/hu-10-vocabulary/`, con este design.md (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` (cada paso se registra antes y después de hacerlo, con campos intent/outcome); además, el historial de la rama y la PR.
**Human-in-the-loop:** (1) el usuario aprueba este diseño; (2) merge automático con squash cuando la validación local pasa (autorizado por el usuario el 2026-10-20); (3) después del merge, el usuario revisa la checklist manual que hay en la descripción de la PR. Si algo falla, se abre una issue de bug y la issue original **no** se reabre automáticamente.
**Irreversible actions:** push de la rama y PR (bajo riesgo); merge a `main` (autorizado si la validación local pasa); llamadas reales a Groq/Anthropic solo con `LIVE_API_TESTS=1` y las keys en variables de entorno (nunca en el repo), con un tope de 1 USD por issue sumado en el log. Nunca: crear tags o releases, tocar secretos ni borrar ramas o datos del usuario.
**Kill switch:** interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.

## Outcome
**Resultado:** ✅ éxito, en 2 iteraciones de validación local más el test en vivo (2026-10-21).

| Ítem | Veredicto | Evidencia |
|---|---|---|
| Validación local, iteración 1 | fail | clippy `unnecessary_to_owned` |
| Validación local, iteración 2 | pass | cargo test 116/116, clippy, lint, tsc, vitest 28/28 y arranque |
| Test en vivo | pass (2/3) | con prompt de Whisper 2/3. **Sin prompt también 2/3**: el prompt de Whisper no aportó en esta muestra. Whisper + Claude con vocabulario: 3/3 (informativo) |
| Entrega | merged | PR #26 → `899a32e`, #10 cerrada |

**Aprendizaje:** el criterio "≥ 2/3 con prompt" no mide la mejora; un criterio mejor sería "con vocabulario > sin vocabulario" o un valor de extremo a extremo. Queda anotado para futuras revisiones.
