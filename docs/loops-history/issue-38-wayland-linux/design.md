# Inner Loop: Issue #38 — Atajo global y pegado en Linux Wayland
> **Status:** APPROVED 2026-11-06

**Issue:** [#38](https://github.com/Alexander940/voice-recorder/issues/38)

**Classification:** inner loop. Es una tarea cerrada: en Linux Wayland el atajo global no dispara nada y `enigo` no puede escribir en ventanas Wayland. Se valida con tests ejecutables más una checklist real en la VM, que es el único sitio donde se puede probar.

**Trigger:** orden del usuario ("ejecuta el loop issue-38") una vez que este documento esté en `APPROVED` y se cumplan los prerrequisitos de la sección 0. No depende de ninguna issue abierta.

**Executor:** el agente en el chat (Claude Code en esta sesión o en una que la retome), en esta VM (Ubuntu 26.04, GNOME 50.1, Wayland, VMware). Trabaja en la rama `fix/issue-38-wayland` y abre una sola PR.

**Objective (verifiable):** una PR que cierra #38, mergeada en `main`, en la que:
1. En Linux con sesión Wayland, el atajo se registra mediante el portal XDG `GlobalShortcuts` y alimenta al `HotkeyController` existente (mantener, alternar y cancelar).
2. En Linux con sesión Wayland, el texto dictado se pega en la ventana activa mediante el portal `RemoteDesktop` (vía la feature `libei` de `enigo`). Si el portal no está disponible o el usuario deniega el permiso, el texto queda en el portapapeles y se notifica, usando la ruta `Delivery::InClipboard` que ya existe.
3. En Linux con sesión X11, en Windows y en macOS el comportamiento no cambia: se sigue usando `tauri-plugin-global-shortcut` y `enigo` con xdo.
4. El atajo por defecto en Linux es `Ctrl+Shift+Space` (no empieza por `Ctrl+Alt`, que VMware intercepta).
5. El README documenta el estado del soporte Linux (X11 frente a Wayland) y las dependencias nuevas.

## 0. Prerrequisitos (los hace el usuario, antes de ejecutar)
La VM no tiene toolchain de Rust ni las librerías de sistema de Tauri. Sin esto el oráculo no se puede ejecutar aquí.

```bash
# Librerías de sistema (necesita sudo)
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libxdo-dev libdbus-1-dev patchelf build-essential pkg-config
# Rust (sin sudo)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

Comprobado en la VM (2026-11-06, solo lectura): portal `GlobalShortcuts` v1 y `RemoteDesktop` v2 con teclado disponibles vía `xdg-desktop-portal-gnome` 50.0; `libei1`/`libeis1` 1.5.0 instaladas; `arboard` y `enigo` están en `Cargo.toml` sin las features de Wayland (`wayland-data-control` y `libei`).

## 1. Enfoque técnico (lo que el agente implementa)
- **Detección de sesión.** Función pura `linux_session()` → `X11 | Wayland` a partir de `XDG_SESSION_TYPE` y `WAYLAND_DISPLAY`. Se inyecta como parámetro para poder testearla.
- **Atajo en Wayland.** Nuevo `PortalRegistrar` que implementa el trait `ShortcutRegistrar` existente usando `ashpd::desktop::global_shortcuts`: crea una sesión, llama a `bind_shortcuts` con el atajo preferido en el formato del portal (`CTRL+SHIFT+space`) y traduce las señales `Activated`/`Deactivated` a `ShortcutState::Pressed`/`Released` para `on_shortcut`. La primera vez GNOME muestra un diálogo para confirmar la combinación; eso es parte de la checklist manual.
- **Cancelar en Wayland.** El portal no permite registrar `Esc` solo durante la grabación. En Wayland se registra un segundo atajo fijo de cancelación (`Ctrl+Shift+Escape`) en la misma sesión del portal. El registro dinámico de `Esc` se mantiene en X11/Windows/macOS. El overlay muestra el atajo de cancelación que aplique.
- **Pegado en Wayland.** Activar la feature `libei` de `enigo` en Linux y `wayland-data-control` en `arboard`. `EnigoKeys::new()` ya convierte un fallo de construcción en `Delivery::InClipboard` con notificación; se añade el texto de ayuda para Linux ("permite el control del teclado en el diálogo de GNOME").
- **Atajo por defecto.** `default_shortcut()` devuelve `Ctrl+Shift+Space` en Linux. El atajo de modos pasa a `Ctrl+Shift+M` en Linux por el mismo motivo.
- **Log estructurado.** Al arrancar, el backend de atajos escribe `hotkey: backend=portal|x11 status=bound|error trigger=<atajo>` en `app.log`. El smoke test lo usa como evidencia.

## 2. Oracle
Nunca se da por buena la salida del agente sin estas comprobaciones. Todas se ejecutan en esta VM, en Wayland.

### 2.1 Validación común (ejecutable)
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` en `src-tauri`; `npm run check:structure`, `npm run lint`, `npm run typecheck`, `npm test`. Sin CI (sigue desactivada por consumo de minutos desde 2026-10-20).

### 2.2 Tests nuevos (ejecutables, obligatorios)
| Test | Qué comprueba |
|---|---|
| `linux_session` | `WAYLAND_DISPLAY` o `XDG_SESSION_TYPE=wayland` → Wayland; si no, X11. |
| `default_shortcut_linux` | En Linux el atajo por defecto y el de modos no contienen `Ctrl+Alt`. |
| `portal_trigger_format` | `Ctrl+Shift+Space` → `CTRL+SHIFT+space`; `Ctrl+Shift+Escape` → `CTRL+SHIFT+Escape`; un atajo inválido → `HotkeyError::Invalid`. |
| `portal_events_feed_controller` | `Activated`/`Deactivated` simulados producen las mismas transiciones del `HotkeyController` que `Pressed`/`Released` (mantener, alternar y cancelar con el atajo de cancelación). |
| `portal_bind_error_is_conflict` | Un error del portal al vincular se traduce a `HotkeyError::Conflict` u `Other` con mensaje legible. |
| Existente `unavailable_keyboard_leaves_text_in_clipboard` | Sigue pasando: sin `RemoteDesktop`, el texto queda en el portapapeles y se notifica. |

### 2.3 Smoke test en Wayland (ejecutable)
`scripts/check-wayland.sh`: lanza el binario compilado durante 15 s con la sesión Wayland de la VM, lee `~/.local/share/com.alexander940.voicerecorder/logs/app.log` y exige una línea `hotkey: backend=portal status=bound`. Sale con código distinto de 0 si no aparece o si aparece `status=error`. Se puede ejecutar a mano sobre cualquier build.

### 2.4 Checklist real en la VM (la hace el usuario, antes del merge)
Es el oráculo definitivo: ni los tests ni el smoke pueden pulsar teclas en GNOME ni aceptar los diálogos del portal.
- [ ] Al abrir la app por primera vez, GNOME muestra el diálogo de atajos; al aceptarlo el atajo queda vinculado (`Ajustes → Teclado → Atajos de aplicaciones` lo lista).
- [ ] Mantener `Ctrl+Shift+Space` con gedit enfocado → overlay de grabación; al soltar, el texto dictado aparece en gedit.
- [ ] Lo mismo en modo alternar: pulsar una vez empieza, otra termina.
- [ ] `Ctrl+Shift+Escape` durante la grabación cancela y no pega nada.
- [ ] Funciona también con GNOME Terminal y Firefox enfocados.
- [ ] Si se deniega el permiso de `RemoteDesktop`, el texto queda en el portapapeles y aparece la notificación.
- [ ] En una sesión X11 (otra máquina o `GDK_BACKEND=x11` bajo XWayland para el pegado, si es posible) el atajo antiguo sigue funcionando. Si no hay dónde probarlo, se anota como no verificado en la PR.

## 3. Exit criteria
- **Éxito:** 2.1, 2.2 y 2.3 pasan en local **y** el usuario confirma la checklist 2.4 → merge con squash y cierre de #38.
- **Fallo:** 5 iteraciones fallidas de la validación ejecutable **o** 3 h de reloj de trabajo del agente **o** 2 rondas fallidas de la checklist manual **o** 1 USD de gasto en APIs reales (solo las usa la checklist, con las keys del usuario) → la PR pasa a draft, se comenta en #38 el diagnóstico con el log de ejecuciones y se escala al usuario.
- **Parada por diseño:** si al implementar aparece una limitación del portal que cambie el contrato (por ejemplo GNOME rechaza el atajo de cancelación), el loop se para y se propone una enmienda a este documento; no se improvisa.

## 4. Memory
`docs/loops-history/issue-38-wayland-linux/` con este `design.md` (sección `## Outcome` al cerrar) y `run-<YYYY-MM-DD>.jsonl` con registro write-ahead: cada paso se anota antes (`intent`) y después (`outcome`, con veredicto y evidencia). Tras una compactación de contexto se relee el log, no la memoria de la conversación. Además, el historial de la rama y la PR.

## 5. Human-in-the-loop
1. El usuario aprueba este diseño.
2. El usuario instala los prerrequisitos de la sección 0 (necesitan sudo).
3. El agente itera solo con la validación ejecutable y abre la PR.
4. **El merge no es automático** (diferencia respecto a los loops HU-01…HU-16): el usuario ejecuta la checklist 2.4 en la VM y responde "ok" o describe el fallo. Solo tras el "ok" el agente mergea.
5. Los diálogos del portal de GNOME (vincular atajo, permitir control del teclado) los acepta el usuario; el agente nunca los simula.

## 6. Irreversible actions
- Push de la rama y PR: bajo riesgo, sin aprobación previa.
- Merge a `main`: solo tras el "ok" de la checklist 2.4.
- Instalar paquetes de sistema y Rust: los ejecuta el usuario.
- Nunca: tags, releases, tocar secretos, borrar ramas o datos del usuario, cambiar el updater.

## 7. Kill switch
Interrumpir al agente; o crear `docs/loops-history/DISABLED`, que se comprueba al inicio de cada iteración. Cada efecto secundario (commit, push, comentario, merge) es atómico y verificable tras una interrupción. Una compilación de Tauri lanzada en segundo plano puede seguir tras el interrupt; el agente comprueba `pgrep cargo` antes de retomar.

## Enmienda 1 (2026-11-06): app id para los portales
> **Status:** aplicada durante la ejecución; no cambia objetivo, oráculo ni criterios de salida.

**Problema detectado en el smoke test.** `GlobalShortcuts.CreateSession` responde `NotAllowed: An app id is required`: una app sin sandbox lanzada desde terminal (o desde un `.desktop` con espacios en el nombre, como el que genera el `.deb`) no tiene app id para xdg-desktop-portal.

**Solución.** Antes de usar cualquier portal, la app se registra con `org.freedesktop.host.portal.Registry` (`ashpd::register_host_app`, xdg-desktop-portal ≥ 1.20). El registro exige que exista `<app id>.desktop`, así que si no lo hay en ningún directorio XDG la app crea uno oculto (`NoDisplay=true`) en `~/.local/share/applications/`. Esto cubre también el AppImage, que no instala ningún `.desktop`.

**Desviación de implementación.** El pegado usa `ashpd` con el portal `RemoteDesktop` directamente, en vez de la feature `libei` de `enigo`: así el backend se elige en tiempo de ejecución y la ruta X11 queda intacta (objetivo 3).

## Enmienda 2 (2026-11-06): el overlay roba el foco en Wayland
> **Status:** aplicada tras la primera ronda fallida de la checklist 2.4; no cambia objetivo, oráculo ni criterios de salida.

**Fallo reportado por el usuario (ronda 1 de la checklist).** El atajo ya funciona, pero al terminar la grabación el overlay dice «Pegado» al instante y el texto no aparece en la app ni queda en el portapapeles (lo segundo es normal: la app restaura el portapapeles anterior 250 ms después de pegar).

**Diagnóstico (reproducible sin micrófono).** Con una sonda independiente (`arboard` + portal `RemoteDesktop` enviando `Ctrl+V` y `Enter` a un `zenity --entry`): el pegado funciona en general y tras cualquier combinación de modificadores, pero falla exactamente en las pulsaciones de `Ctrl+Shift+Space` que hacen aparecer el overlay, y vuelve a funcionar en las que lo encuentran ya visible (alternancia fallo/éxito en 4 de 4 intentos; con la app parada siempre funciona). En Wayland xdg-shell no tiene ventanas «sin foco» (`focusable: false` de Tauri es `gtk_window_set_accept_focus`, que el compositor ignora) y GNOME no implementa layer-shell, así que el compositor da el foco al overlay al mapearlo y el `Ctrl+V` del portal llega al overlay.

**Solución.** En Wayland, antes de pegar la app oculta el overlay, espera a que pierda el foco (máx. 500 ms) más 150 ms de margen para que Mutter se lo devuelva a la ventana anterior, y entonces pega. Después no vuelve a mostrar «Pegado»: mapearlo de nuevo le quitaría el foco a la app justo cuando el usuario sigue escribiendo. X11, Windows y macOS no cambian. Documentado en README y CHANGELOG.

**Limitación que queda.** Durante la grabación y el procesado el overlay sí tiene el foco; las teclas que se pulsen en ese intervalo van al overlay. Es aceptable (el usuario está dictando), pero se anota en la PR.

**Hallazgo adicional sobre el portapapeles.** Mutter 50 no anuncia `ext-data-control` ni `wlr-data-control`, así que `arboard` avisa y usa el portapapeles X11 vía XWayland, que Mutter puentea a Wayland: el pegado funciona igual. Que el texto «no quede en el portapapeles» tras pegar es por diseño (la app restaura el contenido anterior 250 ms después). Como medida preventiva, documentada por `arboard` para X11 sin gestor de portapapeles, la app conserva un handle del portapapeles toda la vida del proceso; en esta VM no se pudo reproducir la pérdida (el test manual `clipboard_survives_drop` pasa con y sin el handle), así que no es la causa del fallo reportado.
