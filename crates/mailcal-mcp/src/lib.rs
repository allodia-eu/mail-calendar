//! `mailcal-mcp`: the local Model Context Protocol server for Allodia Mail & Calendar.
//!
//! # Why this exists
//!
//! Whether you can drive your mail from an assistant should not be your mail provider's
//! decision. Today it is: Gmail and Microsoft users get agent tooling because their providers
//! shipped an MCP surface, and everyone on a small IMAP host gets nothing. Allodia already
//! normalises four providers behind one command layer, so exposing that layer turns "your
//! provider decides whether you get agents" into "you do".
//!
//! # Why this is a separate crate
//!
//! The same reason `mailcal-telemetry` is: **a build without it structurally cannot listen.**
//! Not "is configured off"; cannot. The demo, the showcase, the test suites and any future
//! air-gapped build link none of this, so there is no socket to misconfigure. The listener
//! modules are additionally `#[cfg]`-gated to macOS/Windows/Linux, which excludes iOS and
//! Android by construction rather than by a runtime check: those OSes suspend the app, and a
//! server that is asleep when a client connects is worse than no server.
//!
//! # The design rule everything follows from
//!
//! > **Writes go through the same door the user does. Reads take a different one.**
//!
//! Writes dispatch the same core actions the UI dispatches, so an assistant's archive happens in
//! the user's own list, visibly, with the same optimistic hide. Reads must not, because every
//! read-shaped `Intent` moves the user's screen and `Intent::OpenMessage` marks the message read
//! on the server: so "read me that email" would clear an unread badge as a side effect of a
//! question. The read path is `mailcal_app`'s `query_*`, whose two guarantee tests exist
//! precisely to stop a later contributor collapsing this back into one path.
//!
//! # This crate adds no mail logic
//!
//! Ordering, search scope, folder order, the recipient index and every write's semantics live in
//! `mailcal-app`. What lives here is the wire format, the tool surface, and `policy`; the
//! controls that bound what a successful prompt injection can reach. See `docs/mcp.md` for the
//! contract and `policy`'s own documentation for what those controls can and cannot defend
//! against.

use std::sync::{Arc, Mutex, RwLock};

use tokio::{runtime::Handle, task::JoinHandle};

pub mod backend;
pub mod branding;
pub mod config;
pub mod endpoint;
pub mod jsonrpc;
mod listener;
mod modern;
pub mod policy;
pub mod schema;
pub mod session;
pub mod tools;

pub use backend::{AgentDraft, ComposerError, MailBackend};
pub use branding::{SERVER_DESCRIPTION, SERVER_NAME, SERVER_TITLE, SERVER_WEBSITE};
pub use config::{DEFAULT_PAGE, MAX_PAGE, McpConfig};
pub use endpoint::{Endpoint, EndpointError};
pub use session::{
    LEGACY_PROTOCOL_VERSIONS, MODERN_PROTOCOL_VERSIONS, SUPPORTED_PROTOCOL_VERSIONS,
};

#[cfg(test)]
mod tests_fake;
#[cfg(test)]
mod tests_modern;
#[cfg(test)]
mod tests_policy;
#[cfg(test)]
mod tests_protocol;
#[cfg(test)]
mod tests_schema;
#[cfg(test)]
mod tests_server;
#[cfg(test)]
mod tests_tools;

/// The running server, as a handle a host holds.
///
/// Lifecycle mirrors the background-sync manager exactly: a field on the app object, tasks on the
/// same capped runtime, [`apply`](McpServer::apply) is abort-then-respawn, and teardown is
/// dropping the runtime. There is no separate stop/start protocol to get out of step.
pub struct McpServer {
    backend: Arc<dyn MailBackend>,
    handle: Handle,
    /// The user's decisions, shared **live** with every open connection; see
    /// [`tools::SharedConfig`]. Updating this is how a settings change reaches an assistant that
    /// is already connected, which restarting the accept task cannot do.
    config: tools::SharedConfig,
    /// The endpoint currently bound, so [`apply`](McpServer::apply) can tell a change that needs
    /// a new listener from one that only needs a new configuration.
    bound: Mutex<Option<Endpoint>>,
    /// The accept task.
    task: Mutex<Option<JoinHandle<()>>>,
}

impl core::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The backend holds live provider handles and credentials; show none of them.
        f.debug_struct("McpServer")
            .field("running", &self.is_running())
            .finish_non_exhaustive()
    }
}

impl McpServer {
    /// Builds a stopped server over `backend`, spawning onto `handle` when it is started.
    #[must_use]
    pub fn new(backend: Arc<dyn MailBackend>, handle: Handle) -> Self {
        Self {
            backend,
            handle,
            config: Arc::new(RwLock::new(Arc::new(McpConfig::default()))),
            bound: Mutex::new(None),
            task: Mutex::new(None),
        }
    }

    /// (Re)applies `config`.
    ///
    /// **Publishing the configuration comes first, and reaches connections that are already
    /// open.** That ordering is the whole point: an MCP client opens one connection and holds it
    /// for the session, so a change that only took effect for *newly accepted* connections meant
    /// ticking an account did nothing until the app restarted, and unticking one did not revoke a
    /// live assistant's access at all. Restarting the accept task cannot fix that, because it
    /// leaves existing connection tasks alone.
    ///
    /// The listener is then restarted **only if the endpoint moved**, or if nothing is running.
    /// Tearing it down for an account tick would churn the socket file for no reason, and would
    /// not have propagated the change anyway.
    ///
    /// Called on every settings change, so it must be cheap and idempotent. A config with no
    /// endpoint, or one whose endpoint does not validate, leaves the server stopped: the correct
    /// reading of "the user has not turned this on".
    pub fn apply(&self, config: &McpConfig) {
        *self.config.write().expect("mcp-config lock poisoned") = Arc::new(config.clone());

        let desired = match config.endpoint.as_deref() {
            None => None,
            Some(raw) => match Endpoint::parse(raw) {
                Ok(endpoint) => Some(endpoint),
                Err(err) => {
                    // The endpoint string never reaches the log; it contains the user's home
                    // directory, and therefore usually their name.
                    log::warn!("mcp: not listening; {err}");
                    None
                }
            },
        };
        let unchanged = {
            let bound = self.bound.lock().expect("mcp-endpoint mutex poisoned");
            *bound == desired && self.is_running()
        };
        if unchanged {
            log::info!("mcp: applied updated settings to the running server");
            return;
        }

        let previous = self.abort_task();
        (*self.bound.lock().expect("mcp-endpoint mutex poisoned")).clone_from(&desired);
        let Some(endpoint) = desired else {
            return;
        };
        let backend = Arc::clone(&self.backend);
        let config = Arc::clone(&self.config);
        let task = self.handle.spawn(async move {
            // Wait for the previous listener to finish releasing the endpoint before touching
            // it. `abort()` is cooperative: the old task is parked in `accept()` and does not
            // unwind until it is next polled, so a new listener that probed immediately would
            // find its own predecessor still answering and correctly (but uselessly) conclude
            // that another instance owns the socket. Awaiting the handle is deterministic where
            // a sleep would be a guess.
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            listener::run(backend, config, endpoint).await;
        });
        *self.task.lock().expect("mcp-task mutex poisoned") = Some(task);
    }

    /// Stops the listener, if one is running. Existing connections end when their peer closes.
    pub fn stop(&self) {
        *self.bound.lock().expect("mcp-endpoint mutex poisoned") = None;
        drop(self.abort_task());
    }

    /// Aborts the running listener and hands back its handle, so a caller that is about to bind
    /// the same endpoint can wait for it to let go.
    fn abort_task(&self) -> Option<JoinHandle<()>> {
        let task = self.task.lock().expect("mcp-task mutex poisoned").take()?;
        task.abort();
        log::info!("mcp: stopped listening");
        Some(task)
    }

    /// Whether a listener task is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.task
            .lock()
            .expect("mcp-task mutex poisoned")
            .as_ref()
            .is_some_and(|task| !task.is_finished())
    }
}
