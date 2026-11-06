//! Detección del entorno gráfico (issue #38).
//!
//! En Linux, los atajos globales y la simulación de teclado solo funcionan por
//! X11 con `tauri-plugin-global-shortcut` y `enigo`. En una sesión Wayland hay
//! que pasar por los portales XDG (`GlobalShortcuts` y `RemoteDesktop`).

/// Servidor gráfico de la sesión en Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxSession {
    X11,
    Wayland,
}

/// Decide la sesión a partir de las variables de entorno. `WAYLAND_DISPLAY`
/// manda: bajo XWayland `DISPLAY` también está definida, así que no sirve
/// para distinguir.
pub fn linux_session_from(
    wayland_display: Option<&str>,
    session_type: Option<&str>,
) -> LinuxSession {
    let wayland = wayland_display.is_some_and(|d| !d.trim().is_empty())
        || session_type.is_some_and(|t| t.trim().eq_ignore_ascii_case("wayland"));
    if wayland {
        LinuxSession::Wayland
    } else {
        LinuxSession::X11
    }
}

/// Sesión actual, leída del entorno del proceso.
pub fn linux_session() -> LinuxSession {
    linux_session_from(
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
    )
}

/// `true` solo en Linux con sesión Wayland: ahí se usan los portales XDG.
pub fn uses_portals() -> bool {
    cfg!(target_os = "linux") && linux_session() == LinuxSession::Wayland
}

/// Los portales gestionan permisos por app id, y una app sin sandbox lanzada
/// desde una terminal no tiene ninguno: `GlobalShortcuts` responde
/// "An app id is required". Se declara con el portal `Registry`
/// (xdg-desktop-portal ≥ 1.20) antes de usar cualquier otro. Si no existe,
/// se avisa en el log y se confía en el que deduzca el sistema.
#[cfg(target_os = "linux")]
pub fn register_portal_app(identifier: &str) {
    if !uses_portals() {
        return;
    }
    // El registro exige un `<app id>.desktop` visible para GIO. El .deb instala
    // "Voice Recorder.desktop" y el AppImage ninguno, así que se crea uno
    // oculto en el directorio del usuario si no existe en ningún sitio.
    match desktop::ensure(identifier) {
        Ok(Some(path)) => tracing::info!("portal: desktop_file={} status=created", path.display()),
        Ok(None) => {}
        Err(e) => tracing::warn!("portal: desktop_file status=error reason={e}"),
    }
    let id = identifier.to_string();
    let result = std::thread::spawn(move || {
        tauri::async_runtime::block_on(async move {
            let app_id = ashpd::AppID::try_from(id.as_str())?;
            ashpd::register_host_app(app_id).await
        })
    })
    .join()
    .unwrap_or(Err(ashpd::Error::NoResponse));
    match result {
        Ok(()) => tracing::info!("portal: app_id={identifier} status=registered"),
        Err(e) => tracing::warn!("portal: app_id={identifier} status=error reason={e}"),
    }
}

/// Archivo `.desktop` con el nombre del app id (Linux).
#[cfg(target_os = "linux")]
pub mod desktop {
    use std::path::{Path, PathBuf};

    /// Directorios XDG donde buscar `applications/<id>.desktop`, el del
    /// usuario primero.
    pub fn data_dirs() -> Vec<PathBuf> {
        let home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
        let system = std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        home.into_iter()
            .chain(
                system
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from),
            )
            .collect()
    }

    /// Dónde está `<id>.desktop`, si existe en algún directorio.
    pub fn find(id: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
        let name = format!("{id}.desktop");
        dirs.iter()
            .map(|d| d.join("applications").join(&name))
            .find(|p| p.is_file())
    }

    /// Entrada oculta (`NoDisplay`): solo sirve para que los portales
    /// identifiquen la app; el menú ya tiene la del instalador.
    pub fn entry(exec: &Path) -> String {
        let exec = exec.to_string_lossy();
        let exec = if exec.contains([' ', '"']) {
            format!("\"{}\"", exec.replace('"', "\\\""))
        } else {
            exec.into_owned()
        };
        format!(
            "[Desktop Entry]\nType=Application\nName=Voice Recorder\nComment=Dictado por voz con Groq Whisper y Claude\nExec={exec}\nIcon=voice-recorder\nCategories=Utility;\nNoDisplay=true\n"
        )
    }

    /// Crea `<id>.desktop` en el directorio del usuario si no existe en
    /// ninguno. Devuelve la ruta creada, o `None` si ya había uno.
    pub fn ensure(id: &str) -> std::io::Result<Option<PathBuf>> {
        let dirs = data_dirs();
        if find(id, &dirs).is_some() {
            return Ok(None);
        }
        let home = dirs
            .first()
            .ok_or_else(|| std::io::Error::other("sin HOME ni XDG_DATA_HOME"))?;
        let dir = home.join("applications");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{id}.desktop"));
        std::fs::write(&path, entry(&std::env::current_exe()?))?;
        Ok(Some(path))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn finds_the_desktop_file_in_the_first_dir_that_has_it() {
            let a = tempfile::tempdir().unwrap();
            let b = tempfile::tempdir().unwrap();
            let id = "com.example.app";
            assert_eq!(find(id, &[a.path().into(), b.path().into()]), None);
            let apps = b.path().join("applications");
            std::fs::create_dir_all(&apps).unwrap();
            std::fs::write(apps.join("com.example.app.desktop"), "x").unwrap();
            assert_eq!(
                find(id, &[a.path().into(), b.path().into()]),
                Some(apps.join("com.example.app.desktop"))
            );
            assert_eq!(find("otra.app", &[b.path().into()]), None);
        }

        #[test]
        fn entry_is_hidden_and_quotes_paths_with_spaces() {
            let plain = entry(Path::new("/usr/bin/voice-recorder"));
            assert!(plain.starts_with("[Desktop Entry]\n"));
            assert!(plain.contains("Exec=/usr/bin/voice-recorder\n"));
            assert!(plain.contains("NoDisplay=true"));
            assert!(plain.contains("Name=Voice Recorder"));
            let spaced = entry(Path::new("/home/u/Voice Recorder.AppImage"));
            assert!(spaced.contains("Exec=\"/home/u/Voice Recorder.AppImage\"\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_display_wins() {
        assert_eq!(
            linux_session_from(Some("wayland-0"), Some("x11")),
            LinuxSession::Wayland
        );
        assert_eq!(
            linux_session_from(Some("wayland-0"), None),
            LinuxSession::Wayland
        );
    }

    #[test]
    fn session_type_wayland_without_display_is_wayland() {
        assert_eq!(
            linux_session_from(None, Some("wayland")),
            LinuxSession::Wayland
        );
        assert_eq!(
            linux_session_from(Some(""), Some("Wayland")),
            LinuxSession::Wayland
        );
    }

    #[test]
    fn otherwise_is_x11() {
        assert_eq!(linux_session_from(None, None), LinuxSession::X11);
        assert_eq!(linux_session_from(None, Some("x11")), LinuxSession::X11);
        assert_eq!(
            linux_session_from(Some("  "), Some("tty")),
            LinuxSession::X11
        );
    }

    #[test]
    fn portals_only_on_linux() {
        if !cfg!(target_os = "linux") {
            assert!(!uses_portals());
        }
    }
}
