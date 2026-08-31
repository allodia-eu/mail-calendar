//! Accepting connections, and the one rule that governs it.
//!
//! # Never steal the socket
//!
//! A stale Unix socket file looks exactly like a live one. The lazy fix; `unlink()` then
//! `bind()`, is wrong in the case that matters: if a *second* copy of the app starts while the
//! first is running, it deletes the running instance's socket, binds its own, and every MCP
//! client silently reconnects to the wrong process. The user sees nothing.
//!
//! So startup **connects first**. A successful connect means somebody is home: log it and stay
//! off. `ECONNREFUSED`/`ENOENT` means the file is a leftover from a crash: unlink and bind.
//! Windows gets this for free from `first_pipe_instance(true)`, which turns a name collision
//! into a loud startup failure rather than a silent hijack.
//!
//! # iOS and Android are excluded by construction
//!
//! There are two `run` functions and the `#[cfg]` picks one. On a mobile target the whole
//! listener (not merely its invocation) is absent from the binary, so there is no socket code
//! to reach even by mistake. That is the right shape: those OSes suspend the app, and a server
//! that is asleep when a client connects is worse than no server at all.

use std::sync::Arc;

use crate::{backend::MailBackend, endpoint::Endpoint, tools::SharedConfig};

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
mod unix;
#[cfg(windows)]
mod windows;

/// Runs the accept loop for `endpoint` until the task is aborted.
///
/// Never returns in the normal case. Returns early (permanently, for this configuration) when
/// the endpoint cannot be bound, which is deliberate: a listener that retries a bind it will
/// never win is a log-spam generator, and the two reasons a bind fails here (another instance
/// owns it, or the path is unusable) are both resolved by the user doing something, not by time
/// passing.
#[cfg(any(all(unix, not(any(target_os = "ios", target_os = "android"))), windows))]
pub(crate) async fn run(backend: Arc<dyn MailBackend>, config: SharedConfig, endpoint: Endpoint) {
    let ctx = crate::session::context(backend, config);
    match endpoint {
        #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
        Endpoint::Unix(path) => unix::run(ctx, &path).await,
        #[cfg(windows)]
        Endpoint::Pipe(name) => windows::run(ctx, &name).await,
        // A pipe name on a Unix host, or a socket path on Windows: a host bug, not a user one.
        #[allow(unreachable_patterns)]
        other => {
            let _ = ctx;
            log::warn!("mcp: endpoint kind {other:?} is unsupported here: not listening");
        }
    }
}

/// The mobile build's `run`: there is no listener to start.
#[cfg(not(any(all(unix, not(any(target_os = "ios", target_os = "android"))), windows)))]
pub(crate) async fn run(
    _backend: Arc<dyn MailBackend>,
    _config: SharedConfig,
    _endpoint: Endpoint,
) {
    log::info!("mcp: this platform hosts no server: not listening");
}

/// Serves one accepted connection. Shared by both listeners so the session logic has exactly one
/// entry point regardless of transport.
#[cfg(any(all(unix, not(any(target_os = "ios", target_os = "android"))), windows))]
pub(crate) fn spawn_connection<S>(ctx: crate::tools::ToolContext, stream: S)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        crate::session::serve(ctx, stream).await;
    });
}
