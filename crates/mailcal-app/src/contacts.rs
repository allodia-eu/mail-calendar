//! Contacts: syncing every account's address books, and projecting the engine's unified
//! people into the snapshot a host renders.
//!
//! The shape mirrors [`crate::calendar_ops`]; sync each account's providers, rebuild, signal
//! the surface; with one difference worth knowing before reading it:
//!
//! **The people index is global, not per-account.** Contacts are deduplicated *across*
//! accounts (the engine joins source cards on shared canonical email), so there is no
//! per-account contacts snapshot to assemble and merge the way the mailbox list does. Every
//! account's cards feed one index, and one `people_page` read returns the finished article.
//! That is why the rebuild below is a single query after the loop rather than a fold over it.

use std::{sync::atomic::Ordering, time::Instant};

use engine_api::{ContactSourceClass, PeopleQuery, Provider};
use mailcal_viewmodel::{ContactDetail, ContactsSnapshot, contacts};

use crate::{App, Surface};

/// How many people one snapshot holds.
///
/// The engine caps a page at 200 and the host renders an A–Z list it scrolls, so this is the
/// ceiling on what a single pull can show rather than a paging window: the list has no
/// "show more" yet. Sized to cover an ordinary personal address book whole; a directory
/// larger than this is why [`ContactsSnapshot`] is rebuilt behind a search query rather than
/// paged (searching narrows in the engine, so a match beyond the cap is still findable).
const CONTACTS_LIMIT: usize = 200;

impl<P: Provider> App<P> {
    /// Syncs every account's address books, then rebuilds the contacts snapshot.
    ///
    /// A provider that fails is skipped rather than aborting the pass: one unreachable
    /// address book must not cost the user the contacts that did sync. The engine rebuilds
    /// the unified people index at the end of each provider's sync, so the snapshot read
    /// after the loop already reflects everything that landed.
    ///
    /// Public because the reconnect path calls it once every account's providers are live;
    /// the first moment there is anything to sync. Before that it is a no-op against
    /// placeholders, which is how contacts came to be filled by nothing but the user opening
    /// the tab.
    pub async fn refresh_contacts(&self) {
        let sync_start = Instant::now();
        let mut synced = 0_usize;
        let mut unavailable = 0_usize;
        let mut failed = 0_usize;
        let mut sources = 0_usize;
        let mut contributing = 0_usize;
        // Clone the account handles and sync with the read guard released; these are
        // network round-trips and must not hold the lock.
        let accounts = self.account_handles().await;
        // `acct` is a per-pass ordinal, never the account id; ids embed the user's address
        // (`docs/logging.md`). It matches the mail sync's `sync[a{n}]` so one attached log
        // reads across both surfaces: the same account is `a0` in either.
        for (acct, account) in accounts.iter().enumerate() {
            if account.contact_providers.is_empty() {
                log::info!("contacts[a{acct}]: skipped; no contact sources bound");
                continue;
            }
            sources += account.contact_providers.len();
            contributing += 1;
            // Discover the account's address books ONCE, then sync each bound adapter's cards.
            //
            // Not `Engine::sync_contacts`, which is the *account-global* convenience entry: it
            // runs discovery **and** cards per provider. For CardDAV the two scopes differ;
            // `address_book_scope` is account-wide (`CardDavAddressBookList`) while each
            // provider is bound to one book: so an account with four address books would
            // re-list the whole address-book home four times, a PROPFIND and a lease claim
            // each, on every refresh. Cards are unaffected either way.
            if let Some(first) = account.contact_providers.first() {
                let started = Instant::now();
                match self.engine.sync_address_books(first, &account.id).await {
                    Ok(report) => log::debug!(
                        "contacts[a{acct}]: address books +{} -{}{} in {}ms",
                        report.applied.upserted,
                        report.applied.tombstoned,
                        recovery_note(report.cursor_recovered),
                        started.elapsed().as_millis(),
                    ),
                    // Non-fatal like everything else here: the books already discovered on an
                    // earlier pass are still in the store, so their cards can still sync.
                    Err(error) => log::warn!(
                        "contacts[a{acct}]: address-book discovery failed in {}ms: {error}",
                        started.elapsed().as_millis(),
                    ),
                }
            }
            for (index, provider) in account.contact_providers.iter().enumerate() {
                let started = Instant::now();
                match self.engine.sync_contact_cards(provider, &account.id).await {
                    // `unavailable` is a **success** carrying a refusal: the source was reached
                    // and declined (a shared book the account cannot read, a personal Microsoft
                    // account whose directory has no contacts). Folding it into `synced`, which
                    // is what `Ok(_)` did; reports a clean pass over a book that contributed
                    // nothing, and the reason the server gave is the whole diagnosis.
                    Ok((cards, people)) => {
                        if let Some(reason) = &cards.unavailable {
                            unavailable += 1;
                            log::warn!(
                                "contacts[a{acct}]: source[{index}] unavailable in {}ms: {reason}",
                                started.elapsed().as_millis(),
                            );
                        } else {
                            synced += 1;
                            log::debug!(
                                "contacts[a{acct}]: source[{index}] +{} -{} card(s){}, \
                                 {} people at gen {} in {}ms",
                                cards.applied.upserted,
                                cards.applied.tombstoned,
                                recovery_note(cards.cursor_recovered),
                                people.people,
                                people.generation,
                                started.elapsed().as_millis(),
                            );
                        }
                    }
                    // Counted and logged, never surfaced as an error state: this is a *sync*
                    // pass, and a half-synced list is still worth showing. A failed **write**
                    // does reach the user, through `ContactWriteStatus`.
                    Err(error) => {
                        failed += 1;
                        log::warn!(
                            "contacts[a{acct}]: source[{index}] failed in {}ms: {error}",
                            started.elapsed().as_millis(),
                        );
                    }
                }
            }
        }
        if sources == 0 {
            // The single most common live-test question, answered before it is asked: an empty
            // Contacts list here is a *binding* outcome, not a sync one, so the next thing to
            // read is the `connection_info: … contacts_source[…]` lines from connect, not this.
            log::info!(
                "refresh_contacts: no contact sources on any of {} account(s), nothing to sync",
                accounts.len(),
            );
        } else {
            // `{contributing} of {total}` rather than a single account count, because the gap
            // between them is itself a finding: three accounts and one contributing is either
            // expected (two are Graph/Google) or the bug being reported.
            log::info!(
                "refresh_contacts: {sources} source(s) on {contributing} of {} account(s); \
                 {synced} synced, {unavailable} unavailable, {failed} failed in {}ms",
                accounts.len(),
                sync_start.elapsed().as_millis(),
            );
        }
        // A contacts sync changes the exact input every photo answer was derived from, so
        // those answers are now stale; including, and especially, the negative ones. The
        // first snapshot is built before any card has synced, so without this every sender
        // gets recorded as "nobody has a card for them" and *stays* that way for the rest of
        // the session: contacts arrive half a second later and no face ever appears.
        // Give up what was known about photos *before* rebuilding, so the rebuild's own
        // resolution starts from nothing and re-asks. The sync just replaced the index every
        // previous answer came from, and the first snapshot is built before any card exists,
        // so without this every sender is recorded as "nobody has a card" and keeps it.
        //
        // The rebuild's pass is the only one needed: after a contacts sync the sole addresses
        // whose answer can have changed are the ones that now have a card, and every card is a
        // row in the snapshot it is about to build. It republishes the mail list too.
        self.forget_sender_photos();
        self.rebuild_contacts().await;
        // A full refresh clears the last write's word, the way the calendar's does: "Saved" is a
        // sentence about something the user just did, and one still on screen when they come back
        // to Contacts an hour later is about nothing they remember.
        self.set_contact_write_status(crate::ContactWriteStatus::Idle);
    }

    /// Rebuilds the contacts snapshot from the engine's unified people and signals the host.
    ///
    /// Network-free: it reads the already-derived people index, so a search keystroke costs a
    /// store read rather than a sync.
    pub(super) async fn rebuild_contacts(&self) {
        // Read the query generation *before* the store read, and re-check it after; see
        // [`Self::search_contacts`] for what the check is defending against.
        let generation = self.contacts_generation.load(Ordering::SeqCst);
        let text = self.contacts_query();
        // The query's LENGTH, never the query. A contacts search term is a name or an address
        // the user typed; content, and forbidden. The length still separates the two cases
        // that matter when a list looks wrong: an unfiltered read, and a read filtered by
        // something still in the search field.
        let query_chars = text.chars().count();
        let query = PeopleQuery {
            query: text,
            limit: CONTACTS_LIMIT,
            // **Personal cards only.** A corporate tenant contributes thousands of directory
            // people, and this list is capped at a couple of hundred rows; unfiltered, the
            // first Microsoft or Google work account would push the user's own contacts out
            // of their own A-Z list. Directory and mail-history entries still do their jobs
            // elsewhere: they feed recipient autosuggest and the face beside a sender.
            source_class: Some(ContactSourceClass::Personal),
            ..PeopleQuery::default()
        };
        let started = Instant::now();
        let snapshot = match self.engine.people_page(&query).await {
            Ok(page) => contacts::build(&page.people),
            Err(error) => {
                // Keep the snapshot we already have. Replacing it with an empty one makes the
                // host draw its "no contacts yet; they appear once they have synced" empty
                // state, which reads as *your contacts are gone*: the exact misreading the
                // two distinct empty states on that screen exist to avoid. A transient store
                // error is not news the user needs; a blanked list is.
                log::warn!(
                    "rebuild_contacts: people page read failed in {}ms, keeping the last \
                     snapshot: {error}",
                    started.elapsed().as_millis(),
                );
                return;
            }
        };
        if self.contacts_generation.load(Ordering::SeqCst) != generation {
            log::debug!(
                "rebuild_contacts: superseded by a newer query after {}ms, result dropped",
                started.elapsed().as_millis(),
            );
            return;
        }
        log::info!(
            "rebuild_contacts: {} row(s), query_chars={query_chars} in {}ms",
            snapshot.rows.len(),
            started.elapsed().as_millis(),
        );
        // Photos already resolved for the mail list serve here too: the same person, the same
        // face. Anything new goes to the same background pass, which republishes both surfaces.
        let mut snapshot = snapshot;
        let wanted = self.attach_contact_photos(&mut snapshot);
        *self.contacts.lock().expect("contacts mutex poisoned") = snapshot;
        self.observer.surface_changed(Surface::Contacts);
        self.resolve_sender_photos(wanted).await;
    }

    /// The current contacts snapshot for the host to render.
    pub fn contacts(&self) -> ContactsSnapshot {
        self.contacts
            .lock()
            .expect("contacts mutex poisoned")
            .clone()
    }

    /// Sets the contacts search query and rebuilds, or clears it when `query` is empty.
    ///
    /// The filtering runs in the **engine** (name, email, phone, organisation and title), so
    /// every client inherits the same matching rather than each filtering the visible rows
    /// its own way, and a match outside the snapshot's cap is still reachable.
    ///
    /// Each keystroke bumps a generation counter, and [`Self::rebuild_contacts`] drops a
    /// result whose generation is stale. Intents are **spawned**, so two fast keystrokes run
    /// concurrently: without this, a slow read for `a` finishing after the read for `ab` would
    /// overwrite the narrower results and leave the list disagreeing with the search field
    /// until the next keystroke.
    pub(super) async fn search_contacts(&self, query: String) {
        *self
            .contacts_query
            .lock()
            .expect("contacts query mutex poisoned") = query;
        self.contacts_generation.fetch_add(1, Ordering::SeqCst);
        self.rebuild_contacts().await;
    }

    /// The active contacts search query.
    fn contacts_query(&self) -> String {
        self.contacts_query
            .lock()
            .expect("contacts query mutex poisoned")
            .clone()
    }

    /// The detail of one person, by the id a [`ContactRow`] carries.
    ///
    /// Resolves through the engine's alias table, so a row the host is still holding after a
    /// merge retired its id still opens; returning `None` only when the person is genuinely
    /// gone, never merely renumbered.
    ///
    /// [`ContactRow`]: mailcal_viewmodel::ContactRow
    pub async fn contact_detail(&self, id: &str) -> Option<ContactDetail> {
        // A person id is a store-local integer, so it is an id rather than content and may be
        // logged; it is also the only handle that ties a "the sheet opened blank" report to
        // the row the user tapped.
        let Some(person) = id
            .parse::<u64>()
            .ok()
            .and_then(|raw| engine_api::PersonId::new(raw).ok())
        else {
            log::warn!("contact_detail: host passed an id that is not a person id: {id:?}");
            return None;
        };
        let started = Instant::now();
        match self.engine.person(person).await {
            Ok(Some(found)) => {
                // The live source cards, for the one thing the person cannot say: which card
                // an edit would go to. `Person::is_writable` says only that *some* source is
                // writable, and an edit has to name one.
                let sources = self
                    .engine
                    .person_sources(person)
                    .await
                    .unwrap_or_else(|error| {
                        log::warn!("contact_detail: source read failed, offering no edit: {error}");
                        Vec::new()
                    });
                log::debug!(
                    "contact_detail: person {person:?} resolved over {} account(s), \
                     {} editable card(s) in {}ms",
                    found.sources.len(),
                    sources.iter().filter(|source| source.writable).count(),
                    started.elapsed().as_millis(),
                );
                let mut detail = contacts::detail(&found, &sources);
                // The photo the list already resolved, by the address the row was keyed on.
                // Read-only, and queues nothing: this screen is opened from a row that is
                // behind it, so whatever there was to know is known.
                detail.avatar.image_path = detail
                    .emails
                    .first()
                    .and_then(|email| self.resolved_photo(&email.value));
                Some(detail)
            }
            // Not a warning: a row held across a merge whose person was genuinely deleted lands
            // here legitimately. It is still worth a line, because the alternative explanation
            // (the alias table failed to follow a retired id) looks identical from the UI.
            Ok(None) => {
                log::info!(
                    "contact_detail: person {person:?} no longer exists ({}ms)",
                    started.elapsed().as_millis(),
                );
                None
            }
            Err(error) => {
                log::warn!(
                    "contact_detail: lookup failed in {}ms: {error}",
                    started.elapsed().as_millis(),
                );
                None
            }
        }
    }
}

/// The `, cursor recovered` suffix a report earns when an invalid or expired cursor forced a
/// full snapshot re-read.
///
/// Worth its own note rather than a boolean field in the line: a recovery is the difference
/// between a slow sync that is broken and a slow sync that is doing a one-off full pass, and
/// that is exactly the question a duration alone provokes.
fn recovery_note(recovered: bool) -> &'static str {
    if recovered { ", cursor recovered" } else { "" }
}
