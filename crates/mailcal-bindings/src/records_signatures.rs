//! Settings → Signatures as the core decides it (`docs/signatures.md`): the library, each
//! account's two slots, every control on the page and what the last change came to. A client
//! draws this and keeps no rule of its own about it: the two an editor asks before the person
//! presses are the core's functions here.

use crate::ControlState;

/// Which of an account's two signature slots is meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignatureSlotKind {
    /// The signature a brand-new message opens with.
    NewMessage,
    /// The signature a reply or a forward opens with; one slot for both, as in Outlook.
    ReplyForward,
}

/// A signature resolved for a composer (from [`crate::MailcalApp::resolve_signature`] /
/// [`crate::MailcalApp::signature_body`]): both bodies, so the host seeds the editor and the core
/// has the text/plain rendering to send alongside it.
#[derive(uniffi::Record)]
pub struct SignatureBody {
    /// The signature's id, so a host can show which one its picker has selected.
    pub id: String,
    /// The HTML fragment to seed into the editor.
    pub body_html: String,
    /// The plain-text rendering that accompanies it.
    pub body_plain: String,
}

/// Settings → Signatures (pulled after `Surface::Signatures` and `Surface::Settings`). Read from
/// memory, so a UI thread may ask it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SignaturesPage {
    /// The library, in the user's order; also what the composer's picker lists.
    pub signatures: Vec<SignatureEntry>,
    /// Every account used for mail, in the order the host stored them.
    pub accounts: Vec<AccountSignatures>,
    /// The button that opens the editor on a new signature.
    pub add: ControlState,
    /// The id an editor opened on a new signature saves under. It names nothing until the first
    /// save, and saving under it again changes that signature rather than adding another.
    pub new_id: String,
    /// The editor's Save: running while a save runs. A client holds it besides while
    /// [`crate::signature_name_refusal`] refuses the name typed.
    pub save: ControlState,
    /// What the last change came to, until the next one starts.
    pub outcome: Option<SignatureOutcome>,
}

/// One signature in the library.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SignatureEntry {
    /// The signature's opaque id.
    pub id: String,
    /// The user's name for it ("Work", "Personal").
    pub name: String,
    /// Opens the editor on it.
    pub edit: ControlState,
    /// Deletes it, once confirmed: running while its deletion runs.
    pub delete: ControlState,
}

/// One account's two slots.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountSignatures {
    /// The account's id, which `SignatureIntent::Assign` names it by.
    pub account_id: String,
    /// The account's address, its label.
    pub email: String,
    /// The signature a new message opens with.
    pub new_message: SignaturePicker,
    /// The signature a reply or a forward opens with.
    pub reply_forward: SignaturePicker,
}

/// A slot's picker: the library's signatures and None.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SignaturePicker {
    /// A signature in the library, or `None` for no signature, which is also what a slot naming
    /// a deleted signature reads as.
    pub selected: Option<String>,
    pub control: ControlState,
}

/// Why a picture may not go into a signature. The client words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignatureImageRefusal {
    /// Not a PNG, JPEG or GIF picture, which is what every recipient's mail app shows.
    Format,
    /// Larger than `limit_bytes`: a signature rides in every message the account sends.
    TooLarge { limit_bytes: u64 },
}

/// Why a signature was not saved. The client words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignatureRefusal {
    /// Its name is empty or only spaces.
    BlankName,
    /// It was deleted while it was being edited.
    Gone,
    /// A picture in it may not be sent with every message.
    Image { refusal: SignatureImageRefusal },
}

/// Which change to the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignatureChangeKind {
    Save,
    Delete,
}

/// What a change to the library came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignatureResult {
    /// Saved, and written: an editor that saved it closes.
    Saved,
    /// Deleted, and written.
    Deleted,
    /// Not made, for this reason: an editor that saved it stays open and says why.
    Refused { reason: SignatureRefusal },
    /// Made, and the library's file could not be written, so it holds until the app closes. The
    /// failed-save notice says so; an editor that saved it closes.
    NotWritten,
}

/// The last change to the library and what it came to.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SignatureOutcome {
    /// Grows with each change: an editor acts on an outcome newer than the one in force when it
    /// opened, for the signature it edits, once.
    pub serial: u64,
    /// The signature the change named: an editor on a new signature names it by the page's
    /// `new_id` as it was when the editor opened.
    pub id: String,
    pub kind: SignatureChangeKind,
    pub result: SignatureResult,
}

/// Why a picture of `media_type` and `size` bytes may not go into a signature, or `None` when it
/// may. An editor asks it of a picked file before reading it and of a pasted picture before
/// inserting it; a save asks it again of every picture's bytes.
#[uniffi::export]
#[must_use]
pub fn signature_image_refusal(media_type: String, size: u64) -> Option<SignatureImageRefusal> {
    mailcal_app::signature_image(&media_type, size)
        .err()
        .map(Into::into)
}

/// Why a signature named `name` would not be saved, or `None` when it would: the editor holds
/// its Save while this refuses, and the save refuses by the same rule.
#[uniffi::export]
#[must_use]
pub fn signature_name_refusal(name: String) -> Option<SignatureRefusal> {
    mailcal_app::signature_name(&name).err().map(Into::into)
}
