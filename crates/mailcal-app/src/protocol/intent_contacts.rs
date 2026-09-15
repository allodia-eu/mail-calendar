//! The contacts half of the inbound protocol ([`ContactsIntent`]), which [`Intent::Contacts`]
//! carries.
//!
//! Split from [`super::intent`] to keep each file under the 500-line limit, as that module was
//! itself split from [`super`]. Contacts are the family that reads as a unit: one surface, one
//! write path, and a privacy rule of their own (`docs/contacts.md`), so nothing outside them had
//! to move. Nothing about the variants changed in the move.

use mailcal_account::ContactEdit;

/// What the contacts surface asks the runtime to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactsIntent {
    /// Sync every account's address books and refresh the contacts snapshot.
    RefreshContacts,
    /// Narrow the contacts list to people matching `query`, or show all for an empty one.
    ///
    /// The matching happens in the engine (name, email, phone, organisation, title), so a
    /// person outside the snapshot's row cap is still findable, which is why this is an
    /// intent that rebuilds rather than a filter the host applies to the rows it holds.
    SearchContacts {
        /// The search text; empty clears the filter.
        query: String,
    },
    /// Save a new contact into one address book, then refresh the list.
    ///
    /// `account`/`address_book` are the client's picker choice, from
    /// [`App::contact_targets`](crate::App::contact_targets); both `None` files it in the
    /// first writable book on offer, which is the whole picker for a user with one account.
    /// A user with no writable book anywhere is offered no create at all, so this failing for
    /// want of a destination means the client offered something it should not have.
    ///
    /// Awaited inline like the calendar writes, its outcome surfaced through
    /// [`ContactWriteStatus`](crate::ContactWriteStatus). Not durable offline: a failed save
    /// stays failed rather than queueing.
    CreateContact {
        /// The chosen book's owning account, or `None` for the first on offer.
        account: Option<String>,
        /// The chosen book's provider id, or `None` for the first on offer.
        address_book: Option<String>,
        /// The values the form holds.
        edit: ContactEdit,
    },
    /// Edit one **source card** of a person, then refresh the list.
    ///
    /// Named by a card and not by a person, which is the load-bearing half: a person is
    /// several accounts' cards joined on a shared address (`docs/contacts.md` §1), and saving
    /// the merged values would file one account's details in another's address book. A client
    /// takes the pair from
    /// [`ContactDetail::editable_cards`](mailcal_viewmodel::ContactDetail::editable_cards),
    /// asking the user which card when there is more than one.
    ///
    /// The write is a **patch**: only the fields the form actually changed are sent, so an
    /// address's label, an organisation's departments, a postal address and a photo all
    /// survive an edit that did not touch them. An edit that changed nothing sends nothing.
    UpdateContact {
        /// The person whose card this is, as the row carried it. The card is looked up among
        /// that person's sources, so a retired id still opens the card it always meant.
        person: String,
        /// The account holding the card.
        account: String,
        /// The card's provider id.
        card: String,
        /// The values the form holds.
        edit: ContactEdit,
    },
}
