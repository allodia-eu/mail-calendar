//! Resolving sender photos, off the path that draws a row.
//!
//! A row's monogram and colour come from the address alone, so they cost nothing. A
//! *photo* costs a store read and, the first time, a provider fetch, and rows are built
//! inside `rebuild_snapshot`, which must stay store-reads-only. So the two are split: what a
//! row draws is read synchronously from the map below, and anything the map cannot answer is
//! resolved by a background pass that publishes **one** further snapshot when it finishes.
//!
//! The contract is [`docs/avatars.md`](../../../docs/avatars.md).

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use engine_api::{CanonicalEmail, Provider};
use mailcal_viewmodel::view::MailboxListSnapshot;

use crate::App;

/// How many addresses one pass will resolve.
///
/// A bound on the *pass*, not on coverage: whatever is left stays `Unresolved` and the next
/// rebuild picks it up. It exists because a first sync can put thousands of distinct senders
/// in front of us at once, and each unknown one may cost a provider round trip.
const RESOLVE_BATCH: usize = 60;

/// What is known about one address's photo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PhotoState {
    /// Never looked. The next pass will.
    Unresolved,
    /// Looked, and there is nothing to draw: no contact, no photo, or bytes we refused.
    /// Distinct from `Unresolved` so a pass does not ask the same question forever.
    None,
    /// A raster image on disk, already sniffed.
    File(String),
    /// A photo found before the people index it came from was replaced: still drawn, and asked
    /// about again. The answer replaces it, with a changed photo or with none.
    Rechecking(String),
}

/// The photo pass's bookkeeping, behind one lock.
///
/// Locked after `avatar_photos` whenever both are held, never before, so the two cannot deadlock.
#[derive(Debug, Default)]
pub(crate) struct AvatarPass {
    /// A pass is in flight, so the rebuild it publishes does not start another behind it.
    running: bool,
    /// A pass was turned away while one ran. The running pass runs once more for it, over
    /// whatever the published lists still need, so its addresses are not left until some later
    /// rebuild.
    pending: bool,
    /// Bumped each time every answer is re-asked (`recheck_sender_photos`). A pass that read the
    /// people index before a bump writes nothing it found: its answers come from an index that
    /// has since been replaced.
    generation: u64,
}

impl<P: Provider> App<P> {
    /// Fills in each row's photo from what has already been resolved, and returns the
    /// addresses that still need looking up.
    ///
    /// Synchronous and allocation-light on purpose: this runs for every row of every rebuild.
    pub(crate) fn attach_photos(&self, snapshot: &mut MailboxListSnapshot) -> Vec<CanonicalEmail> {
        let mut photos = self.avatar_photos.lock().expect("avatar mutex poisoned");
        let mut wanted = Vec::new();
        for row in &mut snapshot.rows {
            let (address, avatar) = row_sender(row);
            // An address the engine cannot canonicalize (a malformed or absent `From`) has
            // no identity to key a photo by. The monogram already covers it.
            let Ok(canonical) = CanonicalEmail::parse(address) else {
                continue;
            };
            attach_one(&mut photos, canonical, &mut avatar.image_path, &mut wanted);
        }
        wanted
    }

    /// The photo already resolved for `address`, if there is one.
    ///
    /// For a surface built one at a time rather than as a list: the reading header. Read-only
    /// and queues nothing: by the time a message is opened its sender is on screen in the list
    /// behind it, so the answer is already here.
    pub(crate) fn resolved_photo(&self, address: &str) -> Option<String> {
        let canonical = CanonicalEmail::parse(address).ok()?;
        match self
            .avatar_photos
            .lock()
            .expect("avatar mutex poisoned")
            .get(canonical.as_str())
        {
            Some(PhotoState::File(path) | PhotoState::Rechecking(path)) => Some(path.clone()),
            _ => None,
        }
    }

    /// Fills in each contacts row's photo, and returns the addresses still to look up.
    ///
    /// The same map as the mail list, because it is the same question: a person is a person
    /// whether they are a sender or a row in the A-Z list, and resolving them twice would
    /// mean two provider fetches for one face.
    pub(crate) fn attach_contact_photos(
        &self,
        snapshot: &mut mailcal_viewmodel::ContactsSnapshot,
    ) -> Vec<CanonicalEmail> {
        let mut photos = self.avatar_photos.lock().expect("avatar mutex poisoned");
        let mut wanted = Vec::new();
        for row in &mut snapshot.rows {
            // A phone-only contact has no address, so nothing keys a photo. The monogram
            // already covers it.
            let Ok(canonical) = CanonicalEmail::parse(&row.primary_email) else {
                continue;
            };
            attach_one(
                &mut photos,
                canonical,
                &mut row.avatar.image_path,
                &mut wanted,
            );
        }
        wanted
    }

    /// Resolves photos for `addresses`, then publishes one further snapshot if anything
    /// changed.
    ///
    /// **One further snapshot, not one per photo.** A publish per resolved address would
    /// signal every client dozens of times for a single screenful: the signal storm the
    /// body prefetch is shaped to avoid too. Whatever this pass does not reach stays
    /// `Unresolved` and is picked up by the next rebuild.
    pub(crate) async fn resolve_sender_photos(&self, addresses: Vec<CanonicalEmail>) {
        if addresses.is_empty() {
            return;
        }
        let Some(_guard) = self.begin_avatar_pass() else {
            return;
        };
        self.run_passes(addresses).await;
    }

    /// Resolves `addresses`, then once more for every pass turned away meanwhile. The caller holds
    /// the pass guard.
    async fn run_passes(&self, mut addresses: Vec<CanonicalEmail>) {
        loop {
            let generation = self
                .avatar_pass
                .lock()
                .expect("avatar mutex poisoned")
                .generation;
            self.resolve_batch(addresses, generation).await;
            if !std::mem::take(
                &mut self
                    .avatar_pass
                    .lock()
                    .expect("avatar mutex poisoned")
                    .pending,
            ) {
                return;
            }
            addresses = self.still_wanted();
        }
    }

    /// The addresses the published mail and contacts lists still need asked about.
    fn still_wanted(&self) -> Vec<CanonicalEmail> {
        let mut wanted = self.attach_photos(&mut self.mailbox_list.get());
        let mut contacts = self
            .contacts
            .lock()
            .expect("contacts mutex poisoned")
            .clone();
        wanted.extend(self.attach_contact_photos(&mut contacts));
        wanted
    }

    /// One pass over at most [`RESOLVE_BATCH`] addresses, publishing once at the end. Records
    /// nothing once `generation`, read before the people lookup, is no longer current.
    async fn resolve_batch(&self, addresses: Vec<CanonicalEmail>, generation: u64) {
        if addresses.is_empty() {
            return;
        }
        let start = Instant::now();
        let mut seen = HashSet::new();
        let batch: Vec<CanonicalEmail> = addresses
            .into_iter()
            .filter(|email| seen.insert(email.as_str().to_owned()))
            .take(RESOLVE_BATCH)
            .collect();
        let people = match self.engine.people_by_email(&batch).await {
            Ok(people) => people,
            Err(error) => {
                log::warn!(
                    "avatars: people lookup failed for {} address(es): {error}",
                    batch.len()
                );
                return;
            }
        };

        let mut resolved = 0usize;
        let mut found = 0usize;
        for email in &batch {
            // A sender nobody has a card for is the common case, and it is an answer: record
            // it so the next pass does not ask again.
            let answer = match people.get(email) {
                Some(person) => self.photo_for_person(person).await,
                None => Some(PhotoState::None),
            };
            let mut photos = self.avatar_photos.lock().expect("avatar mutex poisoned");
            if self
                .avatar_pass
                .lock()
                .expect("avatar mutex poisoned")
                .generation
                != generation
            {
                log::info!("avatars: every answer was re-asked during the pass; it records none");
                return;
            }
            let state = answer.unwrap_or_else(|| unanswered(photos.get(email.as_str())));
            if matches!(state, PhotoState::File(_)) {
                found += 1;
            }
            resolved += 1;
            photos.insert(email.as_str().to_owned(), state);
        }
        log::info!(
            "avatars: resolved {resolved} address(es), {found} with a photo, in {}ms",
            start.elapsed().as_millis(),
        );
        // Re-attach onto the snapshots already published rather than rebuilding them. The
        // rows, their order and their contents are unchanged: only the faces are new; so
        // re-projecting would repeat every store read to arrive at the same lists. It also keeps
        // this off `rebuild_snapshot`'s call path, which would otherwise recurse into itself
        // through here.
        //
        // Published only when a face on screen differs, judged against the list as published
        // rather than against the map: a rebuild that attached before this pass answered can
        // publish after it, and a re-checked photo that has not changed is no news.
        let current = self.mailbox_list.get();
        let mut snapshot = current.clone();
        let _ = self.attach_photos(&mut snapshot);
        if snapshot != current {
            self.mailbox_list.publish(snapshot);
        }
        self.republish_contacts_with_photos();
    }

    /// Re-attaches photos onto the contacts snapshot and signals that surface if a face changed.
    ///
    /// Separate from the mail list because contacts are not a `Surfaced` cell: the snapshot is
    /// held behind its own mutex and the surface is signalled by hand.
    fn republish_contacts_with_photos(&self) {
        let current = self
            .contacts
            .lock()
            .expect("contacts mutex poisoned")
            .clone();
        let mut snapshot = current.clone();
        let _ = self.attach_contact_photos(&mut snapshot);
        if snapshot == current {
            return;
        }
        *self.contacts.lock().expect("contacts mutex poisoned") = snapshot;
        self.observer.surface_changed(crate::Surface::Contacts);
    }

    /// Asks again about every sender photo, so the next rebuild re-resolves them all.
    ///
    /// Called when contacts sync, because that replaces the people index every answer was
    /// derived from. Re-asking about the *positive* entries too is deliberate and nearly free:
    /// a re-resolve reads the engine's own cache, which needs no provider and no network, and
    /// it is what picks up a photo that changed on the server.
    ///
    /// A photo already found stays drawn while it is asked about (`Rechecking`), so a photo
    /// that has not changed never leaves the row.
    pub(crate) fn recheck_sender_photos(&self) {
        let mut photos = self.avatar_photos.lock().expect("avatar mutex poisoned");
        photos.retain(|_, state| match state {
            PhotoState::File(path) => {
                *state = PhotoState::Rechecking(std::mem::take(path));
                true
            }
            PhotoState::Rechecking(_) => true,
            PhotoState::Unresolved | PhotoState::None => false,
        });
        // Under the map's lock, so a pass checking the generation before it writes cannot slip
        // an answer from the old index in between.
        self.avatar_pass
            .lock()
            .expect("avatar mutex poisoned")
            .generation += 1;
    }

    /// Marks a pass as running, or records it as pending and returns `None` when one already is.
    fn begin_avatar_pass(&self) -> Option<AvatarPassGuard<'_>> {
        let mut pass = self.avatar_pass.lock().expect("avatar mutex poisoned");
        if pass.running {
            pass.pending = true;
            return None;
        }
        pass.running = true;
        Some(AvatarPassGuard {
            pass: &self.avatar_pass,
        })
    }
}

/// Fills one avatar's photo from the map, and queues the address when it is to be asked about.
fn attach_one(
    photos: &mut HashMap<String, PhotoState>,
    canonical: CanonicalEmail,
    image_path: &mut Option<String>,
    wanted: &mut Vec<CanonicalEmail>,
) {
    match photos.get(canonical.as_str()) {
        Some(PhotoState::File(path)) => *image_path = Some(path.clone()),
        Some(PhotoState::Rechecking(path)) => {
            *image_path = Some(path.clone());
            wanted.push(canonical);
        }
        // Cleared, not left: a pass re-attaches onto the snapshot already on screen, and a
        // photo the source has dropped must leave the row.
        Some(PhotoState::None) => *image_path = None,
        Some(PhotoState::Unresolved) | None => {
            *image_path = None;
            // Mark it now so the same address queued from twenty rows is one lookup.
            photos.insert(canonical.as_str().to_owned(), PhotoState::Unresolved);
            wanted.push(canonical);
        }
    }
}

/// Clears the in-flight mark on every exit path, so an error can never leave the pass
/// permanently "already running".
struct AvatarPassGuard<'a> {
    pass: &'a std::sync::Mutex<AvatarPass>,
}

impl Drop for AvatarPassGuard<'_> {
    fn drop(&mut self) {
        self.pass.lock().expect("avatar mutex poisoned").running = false;
    }
}

/// What a lookup that could not be asked leaves an address at.
///
/// A photo being re-checked stays drawn: only an answer replaces it, and this was none. An
/// address never resolved is recorded as having no photo, as an answer would be, so a provider
/// that stays offline is not asked again on every rebuild; the next contacts sync re-asks it.
fn unanswered(previous: Option<&PhotoState>) -> PhotoState {
    match previous {
        Some(PhotoState::Rechecking(path) | PhotoState::File(path)) => {
            PhotoState::File(path.clone())
        }
        _ => PhotoState::None,
    }
}

/// The address a row names and the avatar to fill in for it.
///
/// Every row shape names exactly one sender: a flat row its own, a thread row its latest;
/// so there is nothing here to fail.
fn row_sender(
    row: &mut mailcal_viewmodel::view::SnapshotRow,
) -> (&str, &mut mailcal_viewmodel::Avatar) {
    use mailcal_viewmodel::view::SnapshotRow;
    match row {
        SnapshotRow::Flat(flat) => (flat.from_address.as_str(), &mut flat.avatar),
        SnapshotRow::Thread(thread) => (thread.latest_from_address.as_str(), &mut thread.avatar),
    }
}

#[path = "avatars_lookup.rs"]
mod lookup;

#[cfg(test)]
#[path = "avatars_test_app.rs"]
mod test_app;

#[cfg(test)]
#[path = "avatars_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "avatars_contacts_tests.rs"]
mod contacts_tests;

#[cfg(test)]
#[path = "avatars_sniff_tests.rs"]
mod sniff_tests;
