//! The test agent: a socket a test, a script or a coding agent drives the running app through.
//!
//! Compiled into a debug or `dev-harness` build only, and started only when `MAILCAL_TEST_AGENT`
//! names the socket to listen on. A shipped build carries neither, so no installed copy answers.
//!
//! It speaks the protocol every client's agent shares, `docs/test-agent.md`: one JSON object per
//! line each way. A request names its `verb`; a reply is `{"ok":true,…}` or
//! `{"ok":false,"error":"…","code":"…"}`, and a caller branches on `code`, never on the wording.
//! `verbs` lists every verb with its arguments, so the protocol describes itself.
//!
//! What it adds over the accessibility bus, which stays the judge of what a screen reader gets:
//! - one call walks the whole widget tree in-process, where AT-SPI costs a round trip per node;
//! - a list row, an expander row and a toggle can be activated, which AT-SPI cannot do in GTK;
//! - `settle` waits for the app itself to go quiet, rather than for a guessed number of seconds;
//! - `screenshot` renders a window from inside the app, so it works on every compositor, GNOME's
//!   included, which offers a client no way to read pixels;
//! - `eval` reaches into a web view, which no accessibility tree describes.
//!
//! The socket is created mode 0600 in a directory only its owner can enter; it answers the user who
//! started the app and nobody else.

mod actions;
mod capture;
mod protocol;
mod query;
mod settle;
#[cfg(test)]
pub(crate) mod tests;
mod tree;

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

pub(crate) use settle::surface_changed;

/// The variable that names the socket. Read here and nowhere else.
const SOCKET_VARIABLE: &str = "MAILCAL_TEST_AGENT";

/// Set by `quit`, and acted on by the thread that wrote its reply.
static QUIT_ASKED: AtomicBool = AtomicBool::new(false);

/// Starts listening when `MAILCAL_TEST_AGENT` names a socket, and does nothing otherwise.
pub(crate) fn start() {
    let Some(path) = std::env::var_os(SOCKET_VARIABLE).map(PathBuf::from) else {
        return;
    };
    match listen(&path) {
        Ok(listener) => {
            log::info!("test agent: listening");
            eprintln!("test agent: listening on {}", path.display());
            if let Err(error) = std::thread::Builder::new()
                .name("test agent".to_owned())
                .spawn(move || accept(&listener))
            {
                eprintln!("test agent: no thread to accept on ({error})");
            }
        }
        Err(error) => eprintln!("test agent: cannot listen on {} ({error})", path.display()),
    }
}

/// Binds the socket, replacing one an earlier run left behind, and leaves it to its owner alone.
fn listen(path: &Path) -> std::io::Result<UnixListener> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

fn accept(listener: &UnixListener) {
    for stream in listener.incoming().flatten() {
        let spawned = std::thread::Builder::new()
            .name("test agent client".to_owned())
            .spawn(move || serve(stream));
        if let Err(error) = spawned {
            eprintln!("test agent: no thread for a client ({error})");
        }
    }
}

/// Answers one client, a line at a time, until it hangs up.
fn serve(stream: std::os::unix::net::UnixStream) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        if line.trim().is_empty() {
            continue;
        }
        let reply = answer(line);
        let written = writeln!(writer, "{reply}").and_then(|()| writer.flush());
        if QUIT_ASKED.swap(false, Ordering::SeqCst) {
            gtk::glib::MainContext::default().invoke(|| {
                if let Some(application) = gtk::gio::Application::default() {
                    gtk::gio::prelude::ApplicationExt::quit(&application);
                }
            });
        }
        if written.is_err() {
            return;
        }
    }
}

/// Runs one request on the main thread, where every widget lives, and waits for its reply.
fn answer(line: String) -> String {
    let (reply, replied) = mpsc::channel();
    gtk::glib::MainContext::default().invoke(move || {
        gtk::glib::spawn_future_local(async move {
            let _ = reply.send(protocol::handle(&line).await.to_string());
        });
    });
    replied.recv().unwrap_or_else(|_| {
        protocol::failure("the app stopped before it answered", "gone").to_string()
    })
}
