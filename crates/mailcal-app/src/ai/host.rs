//! The two ports the writing assistant reaches the outside through. This crate opens no socket and
//! holds no keystore: the bindings install an [`AiHost`] at launch, and a build that can sign in
//! to Allodia installs an [`AiRelay`] beside it. A core with neither offers nowhere to send.

use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use mailcal_ai::{AiBackend, HttpTransport};

/// Set when the person stops the work it was handed to; shared by the work, its transport and the
/// queue that set it.
#[derive(Clone, Default)]
pub struct AiCancel(Arc<AtomicBool>);

impl AiCancel {
    /// Whether the work was stopped.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// The flag itself, for `mailcal_ai::LearnOptions::cancel`.
    #[must_use]
    pub fn as_atomic(&self) -> &AtomicBool {
        &self.0
    }

    pub(crate) fn set(&self) {
        self.0.store(true, Ordering::Release);
    }
}

impl fmt::Debug for AiCancel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AiCancel").field(&self.is_set()).finish()
    }
}

/// The platform keystore refused to store or delete the endpoint's key. Carries nothing: the
/// store's own message can name the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyNotStored;

/// What the host does for the writing assistant: the socket, the runtime and the keystore.
///
/// Every method is called on a thread of the assistant's own, never on a UI thread.
pub trait AiHost: Send + Sync {
    /// A transport over the app's TLS policy. A request it is sending when `cancel` is set is
    /// abandoned and answers as no answer.
    fn transport(&self, cancel: AiCancel) -> Box<dyn HttpTransport>;

    /// Runs `work` to its end on the app's runtime, and returns once it has. For the engine's
    /// reads a piece of work needs between its requests.
    fn wait(&self, work: Pin<Box<dyn Future<Output = ()> + Send + '_>>);

    /// Writes the own endpoint's key to the platform keystore under `ai-endpoint`, or takes it
    /// out with `None`.
    ///
    /// # Errors
    ///
    /// [`KeyNotStored`] when the keystore refused.
    fn store_endpoint_key(&self, key: Option<&str>) -> Result<(), KeyNotStored>;
}

/// Whether the relay can be asked now, as the licence side knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayStatus {
    /// Signed in, with a grant the relay takes.
    Ready,
    /// Signed in, and the sign-in has to be made again: the service refused it, or it lacks
    /// `mailcal:ai:use`.
    SignInNeeded,
    /// Nobody is signed in, so the relay is not somewhere to send.
    SignedOut,
}

/// Allodia's relay (`docs/ai.md` rule 2), installed by a build that can sign in.
pub trait AiRelay: Send + Sync {
    /// Whether the relay can be asked now. Read from memory: called while a page is drawn.
    fn status(&self) -> RelayStatus;

    /// A backend that sends to the relay through `transport`. The core puts it behind the gate
    /// as `Destination::AllodiaRelay`; nothing else may claim that destination.
    fn backend(&self, transport: Box<dyn HttpTransport>) -> Box<dyn AiBackend>;
}

/// The ports as installed, and what the offer is decided from.
#[derive(Default)]
pub(crate) struct Ports {
    pub(crate) host: Option<Arc<dyn AiHost>>,
    pub(crate) relay: Option<Arc<dyn AiRelay>>,
    pub(crate) build: super::BuildKind,
    pub(crate) entitlement: super::AiEntitlement,
    /// The own endpoint's key, read from the keystore at launch and replaced by a save.
    pub(crate) key: Option<String>,
}

impl fmt::Debug for Ports {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ports")
            .field("host", &self.host.is_some())
            .field("relay", &self.relay.is_some())
            .field("build", &self.build)
            .field("entitlement", &self.entitlement)
            .field("key", &self.key.is_some())
            .finish()
    }
}
