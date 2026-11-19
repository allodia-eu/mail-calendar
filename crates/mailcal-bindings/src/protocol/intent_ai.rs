//! The writing assistant's half of the FFI intent surface (`docs/ai.md`), behind one `Intent::Ai`
//! variant: a family per surface of the feature, each nested here.

use crate::JurisdictionClass;

/// A change to one of the writing assistant's surfaces.
#[derive(uniffi::Enum)]
pub enum AiIntent {
    /// A change to the own endpoint ([`AiEndpointIntent`]).
    Endpoint { intent: AiEndpointIntent },
    /// A change to a composer's Draft a reply ([`crate::reply_draft::ReplyDraftIntent`]).
    ReplyDraft {
        intent: crate::reply_draft::ReplyDraftIntent,
    },
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
        /// The key as typed, which replaces the kept one; empty keeps none. `None` leaves the kept
        /// key as it is: what a client sends when the person left the key field empty while the
        /// form says one is kept.
        key: Option<String>,
    },
    /// Removes the endpoint and its key.
    Remove,
}
