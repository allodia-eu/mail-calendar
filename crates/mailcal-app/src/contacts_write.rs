//! Creating and editing a contact: the destinations a client may offer, the card an editor
//! is seeded from, and the two writes themselves.
//!
//! Split from [`crate::contacts`], which reads. The shape mirrors [`crate::calendar_ops`]'s
//! writes: await the engine inline, drive a status the host renders, rebuild the surface. Two
//! things are particular to contacts.
//!
//! **A write names a card, never a person.** The list and the detail show *people*, which the
//! engine assembled from the cards several accounts hold. Writing the values on screen back
//! without choosing one of those cards would file the work account's details in the personal
//! account's address book, so an edit carries the account and the card id, and the client asks
//! which when there is more than one ([`ContactDetail::editable_cards`]).
//!
//! **A destination is an address book, and a client may only offer a writable one.** A
//! directory, a suggested-contacts source and a shared book the account may only read are all
//! places contacts come *from*. Offering one as "save to…" produces a save that fails on the
//! server after the user has typed everything in.
//!
//! [`ContactDetail::editable_cards`]: mailcal_viewmodel::ContactDetail::editable_cards

use std::time::Instant;

use engine_api::{
    AccountId, AddressBookId, ContactCard, ContactId, ContactReconciled, ContactsProvider,
    PersonId, Provider,
};
use mailcal_account::{ContactEdit, build_contact_draft, build_contact_patch};

use crate::{
    App, ContactWriteStatus, Surface,
    helpers::{generated_contact_uid, generated_idempotency},
};

/// One address book a new contact may be saved into.
///
/// The label is the book's **server-given name** and nothing else: the account is named by the
/// client, which already maps an account id to the address the user knows it by, and the
/// core's ids are internal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactTarget {
    /// The account holding the book.
    pub account: String,
    /// The book's provider id, passed back on a create.
    pub address_book: String,
    /// The book's display name, empty when the server gave none.
    pub name: String,
    /// Whether the provider calls this the account's default book. A client preselects the
    /// first default it is offered, else the first target.
    pub is_default: bool,
}

impl<P: Provider> App<P> {
    /// Every address book a new contact could be saved into, across every account.
    ///
    /// **Writable ones only.** Ordered by account (in the account list's order) and then by
    /// the engine's own ordering within an account, so a client's picker is stable between
    /// openings. An empty result means this user has nowhere to save a contact, and a client
    /// offers no "new contact" affordance rather than one that cannot succeed.
    ///
    /// Network-free: a store read of what the last sync discovered.
    pub async fn contact_targets(&self) -> Vec<ContactTarget> {
        let mut targets = Vec::new();
        for account in self.account_handles().await {
            // What the *adapter* will accept, which is the authority: a book the server
            // reports as writable is still read-only through an adapter bound elsewhere.
            let destinations = self.engine.contact_destinations(
                account
                    .contact_providers
                    .iter()
                    .map(std::convert::AsRef::as_ref),
            );
            if destinations.is_empty() {
                continue;
            }
            // Names come from the discovered books; a destination carries only the id.
            let books = self
                .engine
                .address_books(&account.id)
                .await
                .unwrap_or_default();
            for destination in destinations {
                let book = books
                    .iter()
                    .find(|book| book.id == destination.address_book);
                targets.push(ContactTarget {
                    account: account.id.as_str().to_owned(),
                    address_book: destination.address_book.as_str().to_owned(),
                    name: book.map(|book| book.name.clone()).unwrap_or_default(),
                    is_default: book.is_some_and(|book| book.is_default),
                });
            }
        }
        log::debug!("contact_targets: {} writable book(s)", targets.len());
        targets
    }

    /// The editable values of one stored card, for seeding an editor.
    ///
    /// Read from the **card**, never from the person the detail screen showed: the person is a
    /// merge, and seeding an editor from it would offer another account's values for saving
    /// into this one's book.
    ///
    /// `None` when the person is gone, or when that account holds no such card.
    pub async fn contact_card(
        &self,
        person: &str,
        account: &str,
        card: &str,
    ) -> Option<ContactEdit> {
        Some(ContactEdit::from_card(
            &self.stored_card(person, account, card).await?,
        ))
    }

    /// Creates a contact in `address_book`, then refreshes the list.
    ///
    /// `account`/`address_book` are the client's picker choice. Both `None` files it in the
    /// first writable book on offer, which is the whole picker for the ordinary user with one
    /// account and one address book.
    pub(super) async fn create_contact(
        &self,
        account: Option<String>,
        address_book: Option<String>,
        edit: ContactEdit,
    ) {
        let Some(target) = self.chosen_target(account, address_book).await else {
            log::warn!("create_contact: no writable address book, nothing was saved");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        let (Ok(account), Ok(book)) = (
            AccountId::try_from(target.account.as_str()),
            AddressBookId::try_from(target.address_book.as_str()),
        ) else {
            log::warn!("create_contact: the chosen destination is not a valid id");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        let draft = match build_contact_draft(book.clone(), &generated_contact_uid(), &edit) {
            Ok(draft) => draft,
            // Refused before anything was sent, and retrying the same form would be refused
            // the same way, so this is its own status rather than a generic failure.
            Err(error) => {
                log::info!("create_contact: refused before sending: {error}");
                self.set_contact_write_status(ContactWriteStatus::Invalid);
                return;
            }
        };
        // The account handle is held for the whole write: the adapters live in it, and the
        // engine takes one by reference.
        let Some(handle) = self.account_handle(&account).await else {
            log::warn!("create_contact: the chosen account is no longer configured");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        let Some(provider) = handle.contact_providers.iter().find(|provider| {
            provider
                .contact_destination()
                .is_some_and(|destination| destination.writable && destination.address_book == book)
        }) else {
            log::warn!("create_contact: the chosen book has no writable adapter bound");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        self.set_contact_write_status(ContactWriteStatus::Saving);
        let started = Instant::now();
        let status = match self
            .engine
            .create_contact(provider, &account, &generated_idempotency(), &draft)
            .await
        {
            Ok(write) => {
                log::info!(
                    "create_contact: saved for [{}] in {}ms",
                    mailcal_account::account_log_handle(account.as_str()),
                    started.elapsed().as_millis(),
                );
                settle(write.reconciled)
            }
            Err(error) => {
                log::warn!(
                    "create_contact: failed for [{}] in {}ms: {error}",
                    mailcal_account::account_log_handle(account.as_str()),
                    started.elapsed().as_millis(),
                );
                ContactWriteStatus::Failed
            }
        };
        self.rebuild_contacts().await;
        self.set_contact_write_status(status);
    }

    /// Edits one stored card, then refreshes the list.
    ///
    /// An edit that changes nothing sends nothing: the patch would be empty, and writing an
    /// empty one would rewrite the card (and bump its revision on every other device) for no
    /// reason.
    pub(super) async fn update_contact(
        &self,
        person: String,
        account: String,
        card: String,
        edit: ContactEdit,
    ) {
        let Some(base) = self.stored_card(&person, &account, &card).await else {
            log::warn!("update_contact: no such card in that account, nothing was saved");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        let Ok(account) = AccountId::try_from(account.as_str()) else {
            log::warn!("update_contact: the host passed an id that is not an account id");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        let patch = match build_contact_patch(&base, &edit) {
            Ok(patch) => patch,
            Err(error) => {
                log::info!("update_contact: refused before sending: {error}");
                self.set_contact_write_status(ContactWriteStatus::Invalid);
                return;
            }
        };
        if patch.fields.is_empty() && patch.kind.is_none() {
            log::info!("update_contact: the form changed nothing, sending no write");
            self.set_contact_write_status(ContactWriteStatus::Saved);
            return;
        }
        let Some(handle) = self.account_handle(&account).await else {
            log::warn!("update_contact: the card's account is no longer configured");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        // A card belongs to every book it is a member of; the adapter is the one bound to a
        // book it is in.
        let Some(provider) = handle.contact_providers.iter().find(|provider| {
            provider.contact_destination().is_some_and(|destination| {
                destination.writable && base.address_books.contains(&destination.address_book)
            })
        }) else {
            log::warn!("update_contact: the card's book has no writable adapter bound");
            self.set_contact_write_status(ContactWriteStatus::Failed);
            return;
        };
        self.set_contact_write_status(ContactWriteStatus::Saving);
        let started = Instant::now();
        let status = match self
            .engine
            .patch_contact(provider, &account, &generated_idempotency(), &base, &patch)
            .await
        {
            Ok(write) => {
                log::info!(
                    "update_contact: saved {} field(s) for [{}] in {}ms",
                    patch.fields.len(),
                    mailcal_account::account_log_handle(account.as_str()),
                    started.elapsed().as_millis(),
                );
                settle(write.reconciled)
            }
            Err(error) => {
                log::warn!(
                    "update_contact: failed for [{}] in {}ms: {error}",
                    mailcal_account::account_log_handle(account.as_str()),
                    started.elapsed().as_millis(),
                );
                ContactWriteStatus::Failed
            }
        };
        self.rebuild_contacts().await;
        self.set_contact_write_status(status);
    }

    /// The most recent contact write's status (pulled after a [`Surface::ContactsStatus`]
    /// signal). See [`ContactWriteStatus`]; in particular, `Failed` does not mean the save was
    /// lost.
    #[must_use]
    pub fn contact_write_status(&self) -> ContactWriteStatus {
        *self
            .contact_write_status
            .lock()
            .expect("contact-write-status mutex poisoned")
    }

    /// Sets the contact-write status, signalling only on a real change so a run of writes does
    /// not churn the host.
    pub(crate) fn set_contact_write_status(&self, status: ContactWriteStatus) {
        let mut current = self
            .contact_write_status
            .lock()
            .expect("contact-write-status mutex poisoned");
        if *current != status {
            *current = status;
            drop(current);
            self.observer.surface_changed(Surface::ContactsStatus);
        }
    }

    /// The stored card `card` in `account`, found among the cards behind `person`.
    ///
    /// Resolved through the **person** rather than by scanning the store, so a retired id a
    /// client is still holding after a merge opens the card it always meant, and so a caller
    /// cannot name a card belonging to somebody else's person.
    async fn stored_card(&self, person: &str, account: &str, card: &str) -> Option<ContactCard> {
        let person = person
            .parse::<u64>()
            .ok()
            .and_then(|raw| PersonId::new(raw).ok())?;
        let account = AccountId::try_from(account).ok()?;
        let card = ContactId::try_from(card).ok()?;
        self.engine
            .person_sources(person)
            .await
            .unwrap_or_default()
            .into_iter()
            .find(|source| source.id.account == account && source.id.contact == card)
            .map(|source| source.card)
    }

    /// The destination the caller chose, or the first one on offer.
    /// The destination the caller chose.
    ///
    /// **A caller that names one and misses gets nothing**, never the first book on offer. The
    /// fallback exists for the caller that named *neither* half, which is a client with one
    /// account saying "wherever contacts go"; extending it to a named-but-absent destination
    /// would file the contact in a different account's address book, and the save would report
    /// success.
    async fn chosen_target(
        &self,
        account: Option<String>,
        address_book: Option<String>,
    ) -> Option<ContactTarget> {
        let targets = self.contact_targets().await;
        if account.is_none() && address_book.is_none() {
            return targets.first().cloned();
        }
        targets
            .iter()
            .find(|target| {
                account
                    .as_deref()
                    .is_none_or(|account| target.account == account)
                    && address_book
                        .as_deref()
                        .is_none_or(|book| target.address_book == book)
            })
            .cloned()
    }
}

/// Turns a write's reconcile outcome into the status the host shows.
///
/// The dangerous mistake here is re-issuing the write, so this never does. Anything but
/// `Applied` means the write **already landed on the server** and only the local copy is
/// stale; the next contacts sync heals it, which is why `Busy` is reported as saved.
fn settle(reconciled: ContactReconciled) -> ContactWriteStatus {
    match reconciled {
        ContactReconciled::Applied(_) => ContactWriteStatus::Saved,
        ContactReconciled::Busy => {
            log::info!("contact write busy; the in-flight sync will settle it");
            ContactWriteStatus::Saved
        }
        ContactReconciled::Failed(error) => {
            log::warn!("contact write reconciliation failed: {error}");
            ContactWriteStatus::Failed
        }
    }
}
