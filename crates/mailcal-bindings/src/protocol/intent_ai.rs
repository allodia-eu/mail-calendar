//! The writing assistant's half of the FFI intent surface (`docs/ai.md`), behind one `Intent::Ai`
//! variant: a family per surface of the feature, each nested here.

use crate::{JurisdictionClass, ReplyDraftIntent, WritingStyleIntent};

/// A change to one of the writing assistant's surfaces.
#[derive(uniffi::Enum)]
pub enum AiIntent {
    /// A change to the own endpoint ([`AiEndpointIntent`]).
    Endpoint { intent: AiEndpointIntent },
    /// A change on Settings → Writing style ([`WritingStyleIntent`]).
    WritingStyle { intent: WritingStyleIntent },
    /// A change to a composer's Draft a reply ([`ReplyDraftIntent`]).
    ReplyDraft { intent: ReplyDraftIntent },
}

/// A change to the own endpoint, from its form under Settings → Advanced. Each runs off the host's
/// thread, its control running meanwhile; what it came to is the form's `outcome`. Asked again
/// while it runs, it is dropped.
#[derive(uniffi::Enum)]
pub enum AiEndpointIntent {
    /// Saves the form's fields as typed; the core trims each.
    Save {
        address: String,
        model: String,
        declared: Option<JurisdictionClass>,
        /// The key as typed, which replaces the kept one. Empty leaves the kept key as it is and
        /// writes nothing to the keystore (`docs/ai.md` rule 7).
        key: String,
    },
    /// Removes the endpoint and its key.
    Remove,
}
