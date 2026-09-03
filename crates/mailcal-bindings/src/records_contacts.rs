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
    /// The source cards behind this person that this account may **edit**.
    ///
    /// An edit names a card, not a person, because a person is several accounts' cards joined
    /// on a shared address. Empty means every source is read-only, and the client shows no
    /// edit affordance rather than one that fails on press. More than one is a merge: the
    /// client asks which card, naming the account (`docs/contacts.md` §3).
    pub editable_cards: Vec<ContactCardRef>,
}

/// One source card, named the way a write names it.
///
/// A card id is unique only within its account, which is why neither half travels alone.
#[derive(uniffi::Record)]
pub struct ContactCardRef {
    /// The account holding the card.
    pub account: String,
    /// The card's provider id.
    pub card: String,
}

/// One address book a new contact may be saved into: a client's "save to…" picker.
///
/// **Writable books only.** A directory, a suggested-contacts source and a shared book the
/// account may only read are places contacts come *from*; offering one produces a save that
/// fails on the server after the user has typed everything in. An empty list means this user
/// has nowhere to save a contact, and the client offers no create at all.
#[derive(uniffi::Record)]
pub struct ContactTarget {
    /// The account holding the book; label it with the address the user knows that account by.
    pub account: String,
    /// The book's provider id, passed back on a create.
    pub address_book: String,
    /// The book's display name, empty when the server gave none.
    pub name: String,
    /// Whether the provider calls this the account's default book. Preselect the first default
    /// on offer, else the first target.
    pub is_default: bool,
}

/// The editable half of one contact card: what an editor's form holds.
///
/// The same fields the detail screen shows, so nothing is editable that the app will not
/// display back. A card's postal addresses, notes, anniversaries and photo are **carried
/// through an edit untouched**; they are absent here because no client draws them yet, not
/// because saving destroys them.
///
/// Two rules the core applies to what you send:
///
/// - The formatted name is derived: the two name parts, else the organisation, else the first
///   address. A contact with none of the three is refused rather than filed as a blank row.
/// - A structured name (`N`) is written only when `surname` is non-empty. A card that carries a
///   formatted name and no structured one seeds its **whole** name into `given_name`, and writing
///   components from that would file "Ada Lovelace" as a first name with no surname in every other
///   client the user owns.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ContactEdit {
    /// The given (first) name.
    pub given_name: String,
    /// The surname (family name).
    pub surname: String,
    /// The organisation name. The only field a company contact needs.
    pub organization: String,
    /// The job title or role.
    pub title: String,
    /// The email addresses, in the order the form lists them. Blank entries are dropped.
    pub emails: Vec<String>,
    /// The phone numbers, in the order the form lists them. Blank entries are dropped.
    pub phones: Vec<String>,
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

/// The state of the most recent contact write (create or edit), pulled after a
/// `Surface::ContactsStatus` signal.
///
/// **`Failed` does not mean the change was lost.** A write whose server call succeeded but
/// whose post-write reconcile could not be confirmed has already landed on the server; only
/// the local view is briefly stale, and the next sync heals it. `Failed` says "we could not
/// confirm this saved", not "your change was rejected".
///
/// `Invalid` is the other sentence: the core refused the form **before** sending anything, so
/// there is something to correct and retrying unchanged would be refused the same way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
pub enum ContactWriteStatus {
    /// No contact write is settling.
    #[default]
    Idle,
    /// A create or edit is in flight, or its reconcile is being retried.
    Saving,
    /// The most recent write settled and the local view holds the server's copy.
    Saved,
    /// The most recent write's server call failed, or its reconcile could not be confirmed.
    Failed,
    /// The form was refused before anything was sent: it names nothing to file the card
    /// under, or carries a value that is not an email address.
    Invalid,
}
