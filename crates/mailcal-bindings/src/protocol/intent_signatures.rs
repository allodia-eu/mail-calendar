//! The signatures half of the FFI intent surface, nested behind one `Intent::Signature` variant
//! as the other families are (`docs/signatures.md`, "Changing the library").

use crate::SignatureSlotKind;

/// A change to the signature library or to an account's signatures, from Settings → Signatures.
#[derive(uniffi::Enum)]
pub enum SignatureIntent {
    /// Stores a signature: one in the library under its id, or a new one under the page's
    /// `new_id`. Runs off the host's thread, its Save running meanwhile; what it came to is the
    /// page's `outcome`. Asked again while it runs, it is dropped.
    Save {
        id: String,
        /// As typed; the core stores it without the spaces around it, and refuses a blank one.
        name: String,
        /// The editor's `signatureBody().body_html`; sanitised as it is stored.
        body_html: String,
        /// The editor's `signatureBody().body_plain`.
        body_plain: String,
    },
    /// Deletes a signature and clears it from every account that used it. Runs as a save does.
    Delete { id: String },
    /// Sets one of an account's slots to a signature in the library, or to none. Applied before
    /// `dispatch` returns; an id naming nothing clears the slot.
    Assign {
        account: String,
        slot: SignatureSlotKind,
        signature: Option<String>,
    },
}
