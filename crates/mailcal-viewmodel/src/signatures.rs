//! The signature library as the core holds it (`docs/signatures.md`): what Settings → Signatures
//! and the composer's picker are drawn from, what runs on it, and what the last change came to.

/// One signature in the user's library, as a settings list or a composer picker renders it.
///
/// Metadata only: the body is fetched one at a time (`App::signature_html`) so a list of ten
/// signatures does not carry ten logos' worth of base64 across the FFI to draw ten names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureRow {
    /// The signature's opaque id; what an assignment and the body fetch name it by.
    pub id: String,
    /// The user's name for it ("Work", "Personal").
    pub name: String,
}

/// Which of an account's two signature slots is meant. Mirrors the persisted
/// `mailcal_account::SignatureSlot`, kept here so the view-model crate stays free of an
/// account-layer dependency (the same shape as [`crate::SwipeDirection`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSlotKind {
    /// The signature a brand-new message opens with.
    NewMessage,
    /// The signature a reply or a forward opens with; one slot for both, as in Outlook.
    ReplyForward,
}

/// One account's signature assignment: which signature it uses for a new message and which for a
/// reply or forward. `None` in a slot means **no signature** there, which is also what a slot
/// naming a deleted signature reads as.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountSignatureRow {
    /// The account's id.
    pub account_id: String,
    /// The account's email address (display label).
    pub email: String,
    /// The signature id used for new messages, or `None`.
    pub new_message: Option<String>,
    /// The signature id used for replies and forwards, or `None`.
    pub reply_forward: Option<String>,
}

/// Why a picture may not go into a signature. The client words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureImageRefusal {
    /// Not a PNG, JPEG or GIF picture, which is what every recipient's mail app shows.
    Format,
    /// Larger than `limit_bytes`: a signature rides in every message the account sends.
    TooLarge {
        /// The largest a picture may be.
        limit_bytes: u64,
    },
}

/// Why a signature was not saved. The client words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureRefusal {
    /// Its name is empty or only spaces.
    BlankName,
    /// It was deleted while it was being edited.
    Gone,
    /// A picture in it may not be sent with every message.
    Image {
        /// Why the picture may not.
        refusal: SignatureImageRefusal,
    },
}

/// Which change to the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureChangeKind {
    /// Storing a signature, new or changed.
    Save,
    /// Deleting one.
    Delete,
}

/// A change to the library that is running, or waiting to run after the one that is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureWork {
    /// The signature it changes.
    pub id: String,
    /// Which change.
    pub kind: SignatureChangeKind,
}

/// What a change to the library came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureResult {
    /// Saved, and written.
    Saved,
    /// Deleted, and written.
    Deleted,
    /// Not made, for this reason.
    Refused {
        /// Why.
        reason: SignatureRefusal,
    },
    /// Made, and the library's file could not be written, so it holds until the app closes. The
    /// failed-save notice says so and offers Try again.
    NotWritten,
}

/// The last change to the library and what it came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureOutcome {
    /// Grows with each change, so a client acts on each outcome once.
    pub serial: u64,
    /// The signature the change named.
    pub id: String,
    /// Which change.
    pub kind: SignatureChangeKind,
    /// What it came to.
    pub result: SignatureResult,
}

/// The signature library and every mail account's assignment, as the core holds them in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignaturesView {
    /// The library, in display order.
    pub library: Vec<SignatureRow>,
    /// One row per account used for mail, in the order the host stored them.
    pub accounts: Vec<AccountSignatureRow>,
    /// The id a new signature is saved under. It names nothing until the first save of a new
    /// signature, and a second save under it changes that signature rather than adding another.
    pub new_id: String,
    /// The change running, then those waiting behind it.
    pub work: Vec<SignatureWork>,
    /// What the last change came to; forgotten when the next one starts.
    pub outcome: Option<SignatureOutcome>,
}
