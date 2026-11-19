//! What the listener is doing, as Settings says it, and the hook that tells the host it moved.
//!
//! A listener reports from inside its own task, which [`McpServer::apply`](crate::McpServer::apply)
//! may abort and replace at any moment. Each start therefore gets a generation, and a report from
//! a listener that has since been replaced is dropped, so a stopped listener cannot write
//! "listening" over its successor's "starting".

use std::sync::{Arc, Mutex};

/// What the local server is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ListenerStatus {
    /// Not asked to listen: the feature is off, or the host set no endpoint.
    #[default]
    Stopped,
    /// Asked to listen; the endpoint is not bound yet.
    Starting,
    /// Bound: an assistant can connect.
    Listening,
    /// Another copy of the app holds the endpoint, and this one stays off rather than take it.
    AnotherInstance,
    /// The endpoint cannot be used for another reason; the log says which.
    Unavailable,
}

/// Why a listener stopped of its own accord.
#[cfg(any(all(unix, not(any(target_os = "ios", target_os = "android"))), windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    AnotherInstance,
    Unavailable,
}

#[cfg(any(all(unix, not(any(target_os = "ios", target_os = "android"))), windows))]
impl From<Refusal> for ListenerStatus {
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::AnotherInstance => Self::AnotherInstance,
            Refusal::Unavailable => Self::Unavailable,
        }
    }
}

type OnChange = Arc<dyn Fn() + Send + Sync>;

/// The status and the hook a change calls.
#[derive(Default)]
pub(crate) struct StatusCell {
    /// The generation of the listener allowed to report, and the status.
    state: Mutex<(u64, ListenerStatus)>,
    on_change: Mutex<Option<OnChange>>,
}

impl StatusCell {
    pub(crate) fn get(&self) -> ListenerStatus {
        self.state.lock().expect("mcp-status mutex poisoned").1
    }

    pub(crate) fn on_change(&self, hook: OnChange) {
        *self.on_change.lock().expect("mcp-status mutex poisoned") = Some(hook);
    }

    /// Sets `status` for no listener, silencing any listener still reporting.
    pub(crate) fn set(&self, status: ListenerStatus) {
        let changed = {
            let mut state = self.state.lock().expect("mcp-status mutex poisoned");
            state.0 += 1;
            std::mem::replace(&mut state.1, status) != status
        };
        if changed {
            self.notify();
        }
    }

    /// Starts a new listener's reports, as [`ListenerStatus::Starting`].
    pub(crate) fn begin(self: &Arc<Self>) -> Reporter {
        let generation = {
            let mut state = self.state.lock().expect("mcp-status mutex poisoned");
            state.0 += 1;
            state.0
        };
        let reporter = Reporter {
            cell: Arc::clone(self),
            generation,
        };
        reporter.report(ListenerStatus::Starting);
        reporter
    }

    fn notify(&self) {
        let hook = self
            .on_change
            .lock()
            .expect("mcp-status mutex poisoned")
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }
}

/// One listener's way to say what it is doing.
pub(crate) struct Reporter {
    cell: Arc<StatusCell>,
    generation: u64,
}

impl Reporter {
    pub(crate) fn report(&self, status: ListenerStatus) {
        let changed = {
            let mut state = self.cell.state.lock().expect("mcp-status mutex poisoned");
            if state.0 != self.generation {
                return;
            }
            std::mem::replace(&mut state.1, status) != status
        };
        if changed {
            self.cell.notify();
        }
    }
}
