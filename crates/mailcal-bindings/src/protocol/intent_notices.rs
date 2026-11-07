//! The notices half of the FFI intent surface, nested behind one `Intent::Notices` variant for
//! the reason `OutboxIntent` is.

use uniffi::Enum;

/// What the person did to a notice, named by its `Notice::id` (`docs/notices.md`).
///
/// Both are ignored for a notice no longer in force, so a host answering a snapshot that has
/// since moved does no harm.
#[derive(Debug, Clone, PartialEq, Eq, Enum)]
pub enum NoticeIntent {
    /// Close a notice whose `dismissible` is set. Ignored for any other.
    Dismiss {
        /// The notice's `id`.
        id: String,
    },
    /// Run a notice's `Retry`: the core refreshes the notice's screen, which dials the account
    /// again, and marks the notice `remedy_failed` if it still stands when that dial ends, which
    /// only the core can tell. A host dispatches this rather than the refresh itself. Ignored for
    /// a notice that does not offer Retry.
    Retry {
        /// The notice's `id`.
        id: String,
    },
    /// The notice's `action` ran and did not complete: a sign-in that failed. A browser the
    /// person closed is not a failure and is not reported, and a Retry is the core's to judge
    /// (`Retry`). Ignored for a notice with no action.
    RemedyFailed {
        /// The notice's `id`.
        id: String,
    },
}
