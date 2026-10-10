//! Starting GTK for the crate's one widget test, and saying why it could not when it cannot.
//!
//! `gtk_init_check` reports only that no display opened. A pipeline that hits it cannot be asked
//! afterwards whether the compositor had gone, refused the connection, or answered without what
//! GTK needs, so the failure says which, from the compositor's socket itself.

use std::{io::ErrorKind, os::unix::net::UnixStream, path::PathBuf};

/// Starts GTK, or fails the test with what the display was doing.
pub(crate) fn init() {
    if let Err(error) = gtk::init() {
        panic!(
            "GTK test requires a display: run it through with-headless-session.sh ({error}). {}",
            display_state(
                std::env::var_os("WAYLAND_DISPLAY").map(PathBuf::from),
                std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
            )
        );
    }
    use_simple_input_method();
}

/// Keeps the desktop's input method out of the widget test, which types nothing through one.
///
/// GTK's Wayland input method segfaults the test on a desktop that runs one, such as GNOME with
/// IBus. Its first focus records the focused context as current before the compositor has
/// announced `text-input-v3`, and until that announcement neither focus-out nor finalize clears
/// it. A test focuses a field and drops its window without running the main loop in between, so
/// the context is freed while still current, and the compositor's `enter` that follows reaches
/// it. A headless compositor runs no input method and never sends `enter`. Reported upstream as
/// <https://gitlab.gnome.org/GNOME/gtk/-/work_items/8458>.
///
/// The setting is read when a text widget picks its input method, so it has to be in place before
/// the first one exists. `GTK_IM_MODULE` in the environment outranks it.
fn use_simple_input_method() {
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_im_module(Some("gtk-im-context-simple"));
    }
}

/// What the Wayland display named by `display` was doing: missing, refusing, or answering.
fn display_state(display: Option<PathBuf>, runtime: Option<PathBuf>) -> String {
    let Some(display) = display else {
        return "WAYLAND_DISPLAY is not set.".to_owned();
    };
    // libwayland reads a relative name against the runtime directory, as this does.
    let socket = if display.is_absolute() {
        display
    } else if let Some(runtime) = runtime {
        runtime.join(display)
    } else {
        return format!(
            "WAYLAND_DISPLAY is {} and XDG_RUNTIME_DIR is not set.",
            display.display()
        );
    };
    match UnixStream::connect(&socket) {
        Ok(_) => format!(
            "{} accepts connections: the compositor is up, and GTK refused what it offers.",
            socket.display()
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            format!(
                "{} does not exist: the compositor is gone.",
                socket.display()
            )
        }
        Err(error) => format!(
            "{} refused the connection ({error}): the compositor is not answering.",
            socket.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{os::unix::net::UnixListener, path::PathBuf};

    use super::display_state;

    #[test]
    fn a_failed_start_says_whether_the_compositor_was_gone_refusing_or_answering() {
        let runtime = std::env::temp_dir().join(format!("gtk-display-{}", std::process::id()));
        std::fs::create_dir_all(&runtime).unwrap();
        let name = PathBuf::from("wayland-test");

        assert!(display_state(None, None).contains("WAYLAND_DISPLAY is not set"));
        assert!(display_state(Some(name.clone()), None).contains("XDG_RUNTIME_DIR is not set"));
        assert!(
            display_state(Some(name.clone()), Some(runtime.clone())).contains("compositor is gone")
        );

        let listener = UnixListener::bind(runtime.join(&name)).unwrap();
        assert!(
            display_state(Some(name.clone()), Some(runtime.clone()))
                .contains("GTK refused what it offers")
        );
        // A socket file nobody listens on any more: what a compositor that crashed leaves behind.
        drop(listener);
        assert!(
            display_state(Some(name), Some(runtime.clone())).contains("refused the connection")
        );
        std::fs::remove_dir_all(runtime).unwrap();
    }
}
