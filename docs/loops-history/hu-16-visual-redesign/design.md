# Inner Loop: HU-16 — Rediseño visual
> **Status:** APPROVED 2026-10-21

**Issue:** [#33](https://github.com/Alexander940/voice-recorder/issues/33)

**Classification:** inner loop. Es una tarea cerrada: implementar 12 pantallas ya diseñadas. Se valida de forma ejecutable comparando cada pantalla con su mockup.

**Trigger:** orden del usuario ("ejecuta el loop HU-16") una vez que este documento esté en `APPROVED`. No depende de ninguna issue abierta (#1–#15 están cerradas).

**Executor:** el agente en el chat (Claude Code en esta sesión o en una que la retome). Trabaja en la rama `feat/hu-16-visual-redesign` y abre una sola PR.

**Objective (verifiable):** una PR que cierra #33, mergeada en `main`. En ella las 12 pantallas implementadas pasan el oráculo visual frente a los mockups aprobados (lienzo https://claude.ai/artifact/7u3ZFbca82EbMULuZ5nrGN), además de la validación común.

## Oracle

### 1. Referencia congelada
- Las 12 fuentes `.dc.html` del lienzo se copian tal cual a `docs/design/mockups/`.
- Se escribe `docs/design/mockups/SHA256SUMS` con sus hashes.
- El oráculo aborta si algún hash no coincide, así que el agente no puede "acercar" el mockup a su implementación.
- La conversión a HTML estático la hace el propio oráculo en cada ejecución, de forma determinista: sustituye cada `{{prop}}` por su valor por defecto de `data-props` y desenvuelve `<x-dc>` y `<helmet>`.

### 2. Captura de la implementación
- **Arnés de pantallas:** la app se sirve con Vite y una ruta de arnés (`?harness=<pantalla>`).
  - El IPC de Tauri se simula con `@tauri-apps/api/mocks` (`mockIPC`).
  - Los fixtures (`src/harness/fixtures.ts`) reproducen el contenido del mockup: mismos dictados, modos, términos, versión y estado de las keys.
  - La plataforma se fuerza a macOS para que los atajos coincidan (`Option+Space`).
  - El arnés no entra en el bundle de producción. Un test lo comprueba buscando en `dist/` que no hay rastro de `harness`.
- **Renderizado:** mockup e implementación se renderizan en Chromium con Playwright.
  - Tamaño del artboard, `deviceScaleFactor: 1`, animaciones desactivadas.
  - Las mismas fuentes Geist locales (`@fontsource`) en los dos lados.
- **Barra de título:** los 36 px superiores de los mockups de ventana son una barra de título simulada; se recortan porque la app usa la barra nativa.
- **Overlay:** se capturan sus 4 estados, uno por arnés, y se comparan con su fila del mockup.
- **Menú de la bandeja:** es nativo y no se puede capturar.
  - Un test de Rust compara los ítems (texto, orden, habilitado, submenú y marca del modo activo) con `docs/design/tray-spec.json`.
  - Ese archivo se extrae del mockup en el paso 0 y queda congelado con un hash en `SHA256SUMS`.

### 3. Criterios por pantalla
Una pantalla pasa solo si cumple los tres. Los umbrales quedan fijados aquí.

| Criterio | Medida | Umbral |
|---|---|---|
| Fidelidad de texto y layout | Todo texto visible del mockup (espacios normalizados) aparece en la implementación. Se compara la posición del centro de su caja, el `font-size`, el `font-weight` y el color calculado. | Todos presentes; centro a ≤ 12 px; tamaño ±1 px; mismo peso; mismo color (ΔE ≤ 3) |
| Fidelidad de píxel | `pixelmatch` con `threshold: 0.1`, ignorando el antialiasing | ≤ 3 % de píxeles distintos |
| Accesibilidad | `@axe-core/playwright` | 0 violaciones `serious` o `critical` |

Comando: `npm run check:visual` (`scripts/visual-oracle.mjs`). Termina con código distinto de 0 si falla cualquier pantalla. Genera `target/visual-report/index.html`, que muestra en paralelo mockup | implementación | diff con las métricas, y `target/visual-report/results.json`.

### 4. Calibración del oráculo
Se hace antes de escribir cualquier pantalla, en un commit propio.
- Mockup contra sí mismo: 0 % de diferencia y todas las pantallas en pass.
- `main` actual contra los mockups: **todas** las pantallas deben dar fail.
- Si alguna de las dos cosas no se cumple, el oráculo está mal: el loop se para y se escala al usuario con una enmienda. Los umbrales nunca se tocan en silencio.
- Tras la calibración se registra en `## Enmienda 1 (2026-10-21): alinear al píxel las unidades del mockup
> **Status:** APPROVED 2026-10-21

**Problema detectado en la iteración 1.** En el lienzo del mockup del overlay, los cuatro estados quedan en posiciones de medio píxel (y = 44,5, 161,5, 254,5 y 347,5 px), porque se alinean en una rejilla de 440 px. Al capturar el elemento, Chromium suaviza sus bordes y su contenido medio píxel. En la ventana real del overlay la píldora está en coordenadas enteras, así que el mockup nunca se puede reproducir píxel a píxel. Es un artefacto del lienzo, no del diseño.

**Evidencia:**
- Implementación tal cual: las 5 capturas del overlay fallan, con entre 6,05 % y 28,81 % de píxeles distintos, 0 textos mal y 0 errores de axe.
- Implementación desplazada 0,5 px, solo para medir y sin commitear: las 5 pasan, con entre 0,00 % y 0,30 %.

**Cambio propuesto en `scripts/visual-oracle.mjs`:**
- Antes de capturar una unidad del **mockup**, se desplaza ese elemento con `transform: translate(-fx, -fy)`, donde `fx` y `fy` son la parte fraccionaria de su posición, para que quede alineado al píxel.
- No cambian la implementación, los umbrales, los mockups ni las pantallas de ventana completa.
- Después del cambio se repite la calibración: `--self` debe dar 15/15 y `main` 0/15. Luego se fija el nuevo `oracle_sha256`.

**Nota de aplicación (2026-10-21):**
- Primero se aplicó tal como estaba escrita, con `transform: translate`. Los tamaños pasaron a coincidir, pero `transform` mueve los glifos medio píxel y deja el texto suavizado: el overlay seguía fallando con entre 3,2 % y 24,1 %.
- Se aplicó entonces con un desplazamiento de layout (`position: relative; left/top`), que alinea la unidad igual que en la ventana real.
- El objetivo aprobado es el mismo: alinear al píxel solo las unidades del mockup, sin cambiar umbrales ni implementación.

**Alternativas descartadas:**
- Desplazar 0,5 px el overlay real: emborrona la píldora en el producto solo para satisfacer al oráculo.
- Aceptar el overlay solo con revisión humana: deja 5 capturas sin oráculo automático.

**Otro hallazgo, resuelto sin tocar el oráculo:** la barra de título simulada de los mockups mide 37 px (36 px más 1 px de borde), no 36. La implementación dibuja esa misma línea de 1 px bajo la barra de título nativa, como se ve en el diseño.

## Outcome` el SHA-256 de `scripts/visual-oracle.mjs`. Desde ese momento el script, los umbrales, `SHA256SUMS`, `tray-spec.json` y `docs/design/deviations.json` son **de solo lectura para el agente**. Cambiarlos requiere una enmienda aprobada por el usuario, y el oráculo comprueba su propio hash al arrancar.

### 5. Desviaciones aceptadas
Están en `docs/design/deviations.json`, congelado. Contenido inicial:
- El badge "Concedido" del micrófono (paso de permisos) se oculta si macOS no permite consultar el estado sin pedir el permiso. Se excluye su texto en esa pantalla.
- En Actualizaciones, las notas `[NOTA DE LA VERSIÓN n]` son marcadores de posición: el fixture usa ese mismo texto.

### 6. Validación común
- Rust: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (incluye el test del menú de la bandeja).
- Frontend: `npm run lint`, `npm run typecheck`, `npm test`, `npm run build`.
- Todo en local (Windows), **sin CI**. El agente nunca da por buena su propia salida sin estas comprobaciones y el oráculo visual.

## Plan de ejecución
Cada paso termina en un commit.
0. **Oráculo primero:** copiar los mockups, `SHA256SUMS`, `tray-spec.json`, `deviations.json`, el arnés vacío y `scripts/visual-oracle.mjs`, y después calibrar.
   - Dependencias de desarrollo nuevas: `playwright` (Chromium, unos 150 MB), `pixelmatch`, `pngjs`, `@axe-core/playwright`.
   - Dependencias de la app: `@fontsource/geist-sans` y `@fontsource/geist-mono`.
1. Tokens CSS, fuentes, la estructura de la ventana con el menú lateral y la tarjeta de estado.
2. Pantallas: Historial → Modos → Vocabulario → Ajustes → Cuentas → Actualizaciones.
3. Onboarding de 5 pasos.
4. Overlay de 4 estados, con los eventos `dictation-state` (processing, pasted, error) desde Rust y el botón cancelar. Para que el botón funcione, el overlay pasa a aceptar el clic sin robar el foco.
5. Menú de la bandeja: estado, "Empezar a dictar" y "Salir de Voice Recorder". También el comando que devuelve la key enmascarada (prefijo y últimos 4 caracteres; nunca la key completa al frontend).

Una **iteración** es una ejecución completa de la validación común y el oráculo visual tras un bloque de cambios. Su resultado (pantallas en pass/fail con métricas) se registra en el run log.

## Exit criteria
- **Éxito:** la validación común y `npm run check:visual` pasan en las 11 capturas de pantalla, y el test de la bandeja pasa.
- **Fallo:** 10 iteraciones sin éxito, **o** 6 h de reloj, **o** una pantalla que falla 4 iteraciones seguidas con la misma métrica. En ese caso:
  - la PR pasa a draft;
  - se comenta en #33 el informe visual y el log;
  - se escala al usuario.
- **Gasto en APIs reales:** 0 USD. Este loop no llama a Groq ni a Anthropic.

## Memory
- Carpeta `docs/loops-history/hu-16-visual-redesign/`:
  - este `design.md`, con la sección `## Outcome` al cerrar, que incluye el hash del oráculo calibrado;
  - `run-<YYYY-MM-DD>.jsonl` con write-ahead: intent antes de cada efecto y outcome después, con las métricas por pantalla de cada iteración.
- El informe visual final, que se adjunta a la PR.
- El historial de la rama y de la PR.

## Human-in-the-loop
1. El usuario aprueba este diseño.
2. **Antes del merge** el usuario revisa el informe visual, publicado como artefacto privado, y la checklist manual en Mac (overlay real, menú de la bandeja real, captura del atajo). Es un cambio respecto a los loops anteriores, que mergeaban solos: la fidelidad visual merece una mirada humana además del oráculo.
3. El merge se hace con squash solo con el OK explícito del usuario.

## Irreversible actions
- **Bajo riesgo:** push de la rama y la PR.
- **Merge a `main`:** solo con el OK del usuario.
- **Instalar dependencias de desarrollo:** reversible.
- **Nunca:** crear tags o releases, tocar secretos, borrar ramas o datos del usuario, ni modificar el oráculo congelado sin una enmienda.

## Kill switch
Interrumpir al agente, o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y se puede verificar tras una interrupción.

## Riesgos aceptados
- Las capturas comparan el renderizado de Chromium, no el WebView real (WKWebView en macOS, WebView2 en Windows). La revisión manual en Mac cubre esa diferencia.
- El menú nativo de la bandeja se valida por estructura, no por aspecto, porque el sistema operativo decide su estilo.

## Outcome
**Calibración del oráculo (paso 0, 2026-10-21):**
- Mockup contra un segundo renderizado de sí mismo: 15/15 pass con 0,00 % de píxeles distintos y 0 textos mal. El renderizado es determinista y las fuentes Geist se cargan en local.
- App de `main` (antes del rediseño) en el arnés: 0/15 pass.
  - Pantallas completas: 4,2–6,2 % de píxeles distintos y 92–100 % de los textos mal.
  - Overlay: faltan los elementos `data-overlay-*`.
- Observación: la métrica de píxeles por sí sola discrimina poco, porque una pantalla sin estilos queda en ~5 % frente al umbral del 3 %. El criterio de textos (posición, tamaño, peso y color de cada texto) es el que más pesa. No se cambia ningún umbral: lo cubre la revisión humana previa al merge.
- **Recalibración tras la enmienda 1:**
  - Mockup contra sí mismo: 15/15 con 0,00 %.
  - App del paso 0, con el oráculo enmendado, en un worktree: 0/15, con los mismos valores que la calibración original.
- oracle_sha256 (original, sustituido por la enmienda 1): 5fdc4723…c38c
- oracle_sha256: `b7aad5c42a9bf97a8f1f048260f6010adc109493b94754063e3b969a79287208`

**Resultado:** ✅ éxito en 5 iteraciones del oráculo, con la enmienda 1 (2026-10-21).

| Ítem | Veredicto | Evidencia |
|---|---|---|
| Iteración 1 | fail | 7/15. El `TITLEBAR` de los mockups medía 37 px; inputs con el color del sistema; estado "Guardado" |
| Iteraciones 2–4 | fail | 10/10 pantallas de ventana pasan; el overlay falla solo por el medio píxel del lienzo; se escala la enmienda 1 |
| Enmienda 1 | approved | Alinear al píxel las unidades del mockup (desplazamiento de layout); recalibración: `--self` 15/15 y app del paso 0 0/15 |
| Iteración 5 | pass | 15/15 (0,00–0,31 %, 0 textos mal, axe 0); validación común completa |
| Revisión humana | ok | El usuario revisó el informe visual (https://claude.ai/artifact/PDFxTzqpneBn3X9CwPDd6B) y la checklist en Mac |
| Entrega | merged | PR #34 → `ee15e50`, #33 cerrada (estado verificado antes de registrar) |

**Riesgos aceptados:**
- El oráculo compara en Chromium; la fidelidad en WKWebView y WebView2 se verificó a mano en Mac.
- La métrica de píxeles sola discrimina poco; el criterio de textos y la revisión humana lo compensan.

