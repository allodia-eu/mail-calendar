//! The backend port and what can go wrong behind it.

use mailcal_jurisdiction::Refused;

use crate::wire::{ChatRequest, ChatResponse};

/// Anything that answers a chat-completions request.
///
/// Implemented privately for an own endpoint here, and for Allodia's relay by `allodia-license`.
/// Nothing in the app holds one directly: it holds a [`GatedBackend`](crate::GatedBackend), whose
/// `chat` runs the jurisdiction gate first, and every function in this crate that dispatches takes
/// that type. An ungated call therefore cannot be written without changing a type.
///
/// **Blocking.** A call waits for the whole answer, bounded by the timeout the implementation
/// applies, so a caller runs it off the main thread.
pub trait AiBackend: Send + Sync {
    /// Sends `request` and waits for the answer.
    ///
    /// # Errors
    ///
    /// Returns an [`AiError`] saying which of the ways a request fails this one took. Never the
    /// server's own sentence: a client words the failure from the variant.
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AiError>;
}

/// Why a request produced no answer.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AiError {
    /// The jurisdiction gate stopped it; nothing left the device.
    #[error("{0}")]
    Refused(Refused),
    /// The relay has no credits left for this person.
    #[error("no credits left")]
    OutOfCredits,
    /// The person's plan does not include AI.
    #[error("not entitled to AI")]
    NotEntitled,
    /// The endpoint refused the key or the sign-in.
    #[error("the endpoint refused the credential")]
    Unauthorized,
    /// The endpoint asked for fewer requests.
    #[error("the endpoint is rate limiting")]
    RateLimited,
    /// The endpoint could not be reached, or did not answer in time.
    #[error("the endpoint could not be reached")]
    Unreachable,
    /// The endpoint answered with a status this crate has no better word for.
    #[error("the endpoint answered {0}")]
    Status(u16),
    /// The answer arrived but was not what was asked for.
    #[error("the answer could not be read")]
    Malformed,
    /// The person stopped it.
    #[error("cancelled")]
    Cancelled,
}
