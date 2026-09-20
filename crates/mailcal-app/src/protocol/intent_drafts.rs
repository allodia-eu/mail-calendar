//! The drafts half of the inbound protocol ([`DraftsIntent`]), which [`Intent::Drafts`]
//! carries.
//!
//! Split from [`super::intent`] to keep each file under the 500-line limit, following
//! [`ContactsIntent`](super::ContactsIntent) and [`OutboxIntent`](super::OutboxIntent).
//! Drafts read as a unit for the reason those do: one composer, one stored copy on the
//! server, and three verbs that mean nothing anywhere else in the app.

use engine_api::AccountId;
use mailcal_composer::ComposerDocument;

use super::ComposerBlob;
// Named only by the intra-doc links below, which rustdoc resolves against this module's scope.
#[allow(unused_imports, reason = "named by intra-doc links on the variants")]
use super::Intent;

/// One open composition, named by the host.
///
/// Minted by the client when it opens a composer and kept for as long as that composer lives,
/// the way [`ReaderId::Window`](crate::ReaderId) names a detached reading window. A desktop
/// has several composers open at once, each saving a different draft, so the id is what keeps
/// their stored copies apart.
///
/// Opaque to the core, which uses it only to look up what it already knows about the
/// composition. It is never sent anywhere: the draft on the server is named by the provider's
/// own key (`docs/drafts.md`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompositionId(String);

impl CompositionId {
    /// Names a composition, rejecting a blank id. The value crosses the FFI from a host, so it
    /// is checked rather than trusted; a blank one would merge every composer's draft into one
    /// entry and each save would supersede a different composer's copy.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty()).then_some(Self(value))
    }

    /// The id as the host gave it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What the composer asks the runtime to do with the draft it is holding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftsIntent {
    /// Store the composer's current content in the account's Drafts folder, replacing what the
    /// previous save of this composition left there.
    ///
    /// Dispatched both by the composer's explicit "Save as draft" and by its idle timer; the
    /// two are the same operation, and the core cannot tell them apart. A save whose content
    /// is unchanged since the last one reaches no server
    /// ([`App::draft_status`](crate::App::draft_status) still moves, so the composer can say
    /// so).
    ///
    /// The draft is built by the same renderer a sent message is, so the quoted original and
    /// the signature are re-sanitised on the way out (`docs/composer-security.md`).
    Save {
        /// Which composer this is, so the save supersedes its own previous copy and no other's.
        composition: CompositionId,
        /// The account to store it under: the composer's From dropdown. `None` derives it the
        /// way a send does ([`Intent::SubmitRichMail`]).
        ///
        /// Fixed by the first save of a composition: a draft already on the server is stored
        /// in one account's folder, and honouring a later change here would leave the first
        /// copy behind in the account it was written to.
        from: Option<AccountId>,
        /// The `To` recipients, comma-separated. May be empty: a draft is unfinished by
        /// definition, and refusing to keep one that has no recipient yet is refusing to keep
        /// the ones most worth keeping.
        to: String,
        /// The `Cc` recipients, comma-separated; may be empty.
        cc: String,
        /// The `Bcc` recipients, comma-separated; may be empty.
        bcc: String,
        /// The subject line; may be empty.
        subject: String,
        /// The shared composer document.
        document: ComposerDocument,
        /// Host-resolved bytes for every attachment handle `document` references.
        blobs: Vec<ComposerBlob>,
    },
    /// Remove this composition's stored draft from the server and forget the composition.
    ///
    /// A composition that was never saved reaches no server: there is nothing there to remove.
    Discard {
        /// The composer whose stored draft to remove.
        composition: CompositionId,
    },
    /// Forget the composition, leaving what is on the server alone: the composer closed and
    /// the draft stays in Drafts where the user will find it.
    ///
    /// Not optional bookkeeping. Without it the core holds a record per composer for the life
    /// of the process, and the next composer the host opens under a **reused** id would
    /// supersede a draft the user had finished with.
    Close {
        /// The composer that closed.
        composition: CompositionId,
    },
}
