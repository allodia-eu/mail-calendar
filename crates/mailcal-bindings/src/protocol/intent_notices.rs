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
    /// The notice's `action` ran and did not complete: a sign-in that failed, a retry that did
    /// not reach the server. A browser the person closed is not a failure and is not reported.
    /// Ignored for a notice with no action.
    RemedyFailed {
        /// The notice's `id`.
        id: String,
    },
}
