//! The contacts and recipient-autosuggest FFI records.
//!
//! Split from `records.rs`, which is at the 500-line limit.
//!
//! Everything here is already **unified**: a [`ContactRow`] is one *person*, not one provider
//! card, and the engine decided which cards make up that person. The client's job is to render
//! it and (when [`ContactRow::account_count`] is above one) say that it is a merge, which is
//! the disclosure rule `docs/contacts.md` binds every platform to.

use crate::records_avatar::Avatar;

/// An immutable contacts snapshot for a host to render, ordered A–Z.
#[derive(uniffi::Record)]
pub struct ContactsSnapshot {
    /// The people, ordered by display name (case-insensitively), then by id.
    pub rows: Vec<ContactRow>,
}

/// One contacts-list row: a unified person reduced to what a list cell shows.
#[derive(uniffi::Record)]
pub struct ContactRow {
    /// The person's stable id; passed back to
    /// [`MailcalApp::contact_detail`](crate::MailcalApp::contact_detail) to open it.
    pub id: String,
    /// The display name, or a placeholder when every source card is nameless.
    pub display_name: String,
    /// The address shown under the name; empty for a person with no email.
    pub primary_email: String,
    /// The A–Z section this row files under; `#` when the name starts with a non-letter.
    pub section: String,
    /// The person's monogram, colour and photo. Decoration; hide it from assistive
    /// technology; the row already announces the name.
    pub avatar: Avatar,
    /// How many **accounts** contributed a card. Above one means this row is a merge, and the
    /// client must say so rather than presenting several cards silently as one.
    pub account_count: u32,
}

/// The detail of one unified person: every value, and which accounts supplied it.
#[derive(uniffi::Record)]
pub struct ContactDetail {
    /// The person's stable id.
    pub id: String,
    /// The display name, matching the row's.
    pub display_name: String,
    /// The person's monogram, colour and photo, matching the row's.
    pub avatar: Avatar,
    /// Every email, with the accounts carrying it.
    pub emails: Vec<ContactValue>,
    /// Every phone number, with the accounts carrying it.
    pub phones: Vec<ContactValue>,
    /// Every organisation, with the accounts carrying it.
    pub organizations: Vec<ContactValue>,
    /// Every title/role, with the accounts carrying it.
    pub titles: Vec<ContactValue>,
    /// The distinct accounts this person was assembled from: the explanation a merged row owes.
    pub accounts: Vec<String>,
}

/// One value on a contact, and the accounts it came from.
#[derive(uniffi::Record)]
pub struct ContactValue {
    /// The value (an email address, phone number, organisation, or title).
    pub value: String,
    /// The distinct accounts carrying it, sorted.
    pub accounts: Vec<String>,
}

/// One ranked recipient suggestion for the composer's To/Cc/Bcc fields.
///
/// Named apart from [`RecipientSuggestion`](crate::RecipientSuggestion), which is a different
/// thing entirely: the To/Cc a **reply** is pre-filled with.
#[derive(uniffi::Record)]
pub struct RecipientMatch {
    /// The address to insert.
    pub email: String,
    /// The display name; empty when only the address is known.
    pub display_name: String,
    /// Whether a saved contact supplies this address, rather than it being known only from
    /// mail the user has sent. A client may mark the two apart but must not hide the
    /// history-only ones; they are usually the most useful.
    pub is_saved: bool,
}
