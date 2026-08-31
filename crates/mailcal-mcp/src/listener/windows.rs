//! The Windows named-pipe listener.
//!
//! # The security model, without `unsafe`
//!
//! `tokio::net::windows::named_pipe`'s `create_with_security_attributes_raw` is a `pub unsafe
//! fn`, and this workspace sets `unsafe_code = "forbid"` (only `mailcal-bindings` opts out, and
//! a socket listener does not belong there). So a hand-rolled DACL is unavailable, and it is
//! also **not needed**:
//!
//! * A pipe created with null security attributes inherits the **creating token's default DACL**,
//!   which grants the user, SYSTEM and Administrators. That is exactly the boundary the Unix side
//!   gets from 0700.
//! * `reject_remote_clients(true)` kills SMB reachability, so `\\host\pipe\…` from another machine
//!   is refused rather than served.
//! * `first_pipe_instance(true)` turns name-squatting into a **loud startup failure** instead of a
//!   silent hijack: the Windows equivalent of "never steal the socket", and it comes for free
//!   because the OS enforces it rather than a probe-then-bind race.
//!
//! That is the whole model, and every part of it is a safe call.

use tokio::net::windows::named_pipe::ServerOptions;

use super::spawn_connection;
use crate::tools::ToolContext;

/// Serves `name` forever, one pipe instance at a time.
///
/// The named-pipe pattern differs from a socket's: each `ServerOptions::create` yields **one**
/// instance, which is handed to a connecting client and then replaced. So the loop creates the
/// next instance before serving the current one, and a client never finds the name unlistened.
pub(super) async fn run(ctx: ToolContext, name: &str) {
    let mut server = match ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(name)
    {
        Ok(server) => server,
        Err(err) => {
            // With `first_pipe_instance`, this is how "another instance already owns the name"
            // surfaces; loudly, at startup, rather than as a silent takeover.
            log::warn!("mcp: could not create the pipe (another instance may own it): {err}");
            return;
        }
    };
    log::info!("mcp: listening for assistant connections");
    loop {
        if let Err(err) = server.connect().await {
            log::warn!("mcp: pipe connect failed: {err}");
            continue;
        }
        // Replace the instance we just handed out, so the name stays listenable.
        let next = match ServerOptions::new()
            .reject_remote_clients(true)
            .create(name)
        {
            Ok(next) => next,
            Err(err) => {
                log::warn!("mcp: could not create the next pipe instance: {err}");
                return;
            }
        };
        let connected = std::mem::replace(&mut server, next);
        spawn_connection(ctx.clone(), connected);
    }
}
