//! The Unix domain socket listener (macOS and Linux).
//!
//! Three things guard the endpoint, and none of them is a token:
//!
//! * the **parent directory is 0700**, so no other user can traverse into it;
//! * the socket itself is chmod'd owner-only after bind, and
//! * every accepted connection's **peer uid is checked**.
//!
//! The uid check is the interesting one. The OS user boundary is already the authenticator here
//! (a same-user process could open `mailcal.sqlite` directly) so `peer_cred()` does not add a
//! new defence so much as turn an *assumption* into something the code verifies. That matters
//! because the assumption is the entire argument for having no token at all, and an argument
//! nobody checks is a belief.
//!
//! The uid to compare against is the **socket file's owner**, which is by definition the process
//! that bound it: us. That is a safe, `std`-only way to learn our own effective uid; `geteuid`
//! itself is `unsafe`, and the workspace forbids `unsafe` outright (a socket listener is not the
//! place to make the exception).

use std::{
    io::ErrorKind,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

use tokio::net::{UnixListener, UnixStream};

use super::spawn_connection;
use crate::tools::ToolContext;

/// Binds `path` and accepts forever.
pub(super) async fn run(ctx: ToolContext, path: &str) {
    let path = Path::new(path);
    if !prepare_directory(path) {
        return;
    }
    if !claim(path).await {
        return;
    }
    let listener = match UnixListener::bind(path) {
        Ok(listener) => listener,
        Err(err) => {
            log::warn!("mcp: could not bind the socket ({}): {err}", err.kind());
            return;
        }
    };
    // Owner-only, in case the process umask was permissive.
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    // We just created this file, so its owner is this process's effective uid.
    let Ok(our_uid) = std::fs::metadata(path).map(|meta| meta.uid()) else {
        log::warn!("mcp: could not read the socket's owner: not listening");
        return;
    };
    // Owning the file means the endpoint disappears when this task does; including when it is
    // *aborted*, because aborting drops the future and runs its destructors. Without it, turning
    // the setting off would leave a socket that a client connects to and then hangs on, which is
    // a worse answer than a refused connection.
    let _endpoint = OwnedSocket(path.to_owned());
    log::info!("mcp: listening for assistant connections");
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                if !peer_is(&stream, our_uid) {
                    log::warn!("mcp: refused a connection from another user");
                    continue;
                }
                spawn_connection(ctx.clone(), stream);
            }
            // A per-connection accept error (a hit descriptor limit, an interrupted syscall) is
            // transient and the listener itself is still good, so keep serving.
            Err(err) => log::warn!("mcp: accept failed: {err}"),
        }
    }
}

/// The socket file, unlinked when the listener task ends or is aborted.
struct OwnedSocket(std::path::PathBuf);

impl Drop for OwnedSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Creates the socket's parent directory and restricts it to this user. Returns whether the
/// directory is usable.
fn prepare_directory(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return true;
    };
    if let Err(err) = std::fs::create_dir_all(parent) {
        log::warn!("mcp: could not create the socket directory: {err}");
        return false;
    }
    if let Err(err) = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)) {
        log::warn!("mcp: could not restrict the socket directory: {err}");
        return false;
    }
    true
}

/// Whether the socket path is ours to bind.
///
/// Connects **first**. Somebody answering means a live instance owns the endpoint and this one
/// must stay off: deleting a running instance's socket would silently steal every client from it,
/// and the user would see nothing. A refused connection or a missing file means the leftover is
/// stale and can be removed. Never `unlink()` unconditionally.
async fn claim(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    match UnixStream::connect(path).await {
        Ok(_) => {
            log::info!("mcp: another instance owns the endpoint: not listening");
            false
        }
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::ConnectionRefused | ErrorKind::NotFound
            ) =>
        {
            match std::fs::remove_file(path) {
                Ok(()) => {
                    log::info!("mcp: removed a stale socket left by a previous run");
                    true
                }
                Err(err) => {
                    log::warn!("mcp: could not remove the stale socket: {err}");
                    false
                }
            }
        }
        // Anything else (a permission error, a path that is not a socket at all) is a state this
        // process should not paper over by deleting a file it does not understand.
        Err(err) => {
            log::warn!("mcp: the endpoint exists but is not usable: {err}");
            false
        }
    }
}

/// Whether the connecting process runs as uid `ours`.
fn peer_is(stream: &UnixStream, ours: u32) -> bool {
    match stream.peer_cred() {
        Ok(cred) => cred.uid() == ours,
        // Fail closed. Both platforms that reach this module can report peer credentials, so a
        // failure here is an anomaly, not a portability gap to be shrugged off.
        Err(err) => {
            log::warn!("mcp: could not read peer credentials: {err}");
            false
        }
    }
}
