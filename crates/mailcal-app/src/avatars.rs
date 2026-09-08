//! Resolving sender photos, off the path that draws a row.
//!
//! A row's monogram and colour come from the address alone, so they cost nothing. A
//! *photo* costs a store read and, the first time, a provider fetch, and rows are built
//! inside `rebuild_snapshot`, which must stay store-reads-only. So the two are split: what a
//! row draws is read synchronously from the map below, and anything the map cannot answer is
//! resolved by a background pass that publishes **one** further snapshot when it finishes.
//!
//! The contract is [`docs/avatars.md`](../../../docs/avatars.md).

use std::{collections::HashSet, time::Instant};

use engine_api::{CanonicalEmail, ContactCard, ContactResource, Provider};
use mailcal_viewmodel::view::MailboxListSnapshot;

use crate::App;

/// How many addresses one pass will resolve.
///
/// A bound on the *pass*, not on coverage: whatever is left stays `Unresolved` and the next
/// rebuild picks it up. It exists because a first sync can put thousands of distinct senders
/// in front of us at once, and each unknown one may cost a provider round trip.
const RESOLVE_BATCH: usize = 60;

/// The largest photo accepted, in bytes.
///
/// A contact photo is drawn at avatar size. Anything past this is a provider handing us a
/// full-resolution image, and decoding it per row costs far more than the monogram it would
/// replace.
const MAX_PHOTO_BYTES: u64 = 2 * 1024 * 1024;

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
            match photos.get(canonical.as_str()) {
                Some(PhotoState::File(path)) => avatar.image_path = Some(path.clone()),
                Some(PhotoState::None) => {}
                Some(PhotoState::Unresolved) | None => {
                    // Mark it now so the same address queued from twenty rows is one lookup.
                    photos.insert(canonical.as_str().to_owned(), PhotoState::Unresolved);
                    wanted.push(canonical);
                }
            }
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
            Some(PhotoState::File(path)) => Some(path.clone()),
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
            match photos.get(canonical.as_str()) {
                Some(PhotoState::File(path)) => row.avatar.image_path = Some(path.clone()),
                Some(PhotoState::None) => {}
                Some(PhotoState::Unresolved) | None => {
                    photos.insert(canonical.as_str().to_owned(), PhotoState::Unresolved);
                    wanted.push(canonical);
                }
            }
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
            let state = match people.get(email) {
                Some(person) => self.photo_for_person(person).await,
                None => PhotoState::None,
            };
            if matches!(state, PhotoState::File(_)) {
                found += 1;
            }
            resolved += 1;
            self.avatar_photos
                .lock()
                .expect("avatar mutex poisoned")
                .insert(email.as_str().to_owned(), state);
        }
        log::info!(
            "avatars: resolved {resolved} address(es), {found} with a photo, in {}ms",
            start.elapsed().as_millis(),
        );
        if found > 0 {
            // Re-attach onto the snapshots already published rather than rebuilding them. The
            // rows, their order and their contents are unchanged: only the faces are new;
            // so re-projecting would repeat every store read to arrive at the same lists. It
            // also keeps this off `rebuild_snapshot`'s call path, which would otherwise
            // recurse into itself through here.
            let mut snapshot = self.mailbox_list.get();
            let _ = self.attach_photos(&mut snapshot);
            self.mailbox_list.publish(snapshot);
            self.republish_contacts_with_photos();
        }
    }

    /// Re-attaches photos onto the contacts snapshot and signals that surface.
    ///
    /// Separate from the mail list because contacts are not a `Surfaced` cell: the snapshot is
    /// held behind its own mutex and the surface is signalled by hand.
    fn republish_contacts_with_photos(&self) {
        let mut snapshot = self
            .contacts
            .lock()
            .expect("contacts mutex poisoned")
            .clone();
        if snapshot.rows.is_empty() {
            return;
        }
        let _ = self.attach_contact_photos(&mut snapshot);
        *self.contacts.lock().expect("contacts mutex poisoned") = snapshot;
        self.observer.surface_changed(crate::Surface::Contacts);
    }

    /// Walks one person's source cards for a usable photo.
    ///
    /// Cache first and without a provider, because most calls are re-reads after a restart:
    /// the engine's cache survives, this in-memory map does not. Only a genuine miss needs a
    /// connected adapter, and an account whose provider is not connected simply contributes
    /// nothing rather than failing the pass.
    async fn photo_for_person(&self, person: &engine_api::Person) -> PhotoState {
        for source in &person.sources {
            let card = match self
                .engine
                .contact_card(&source.account, &source.contact)
                .await
            {
                Ok(Some(card)) => card,
                Ok(None) => continue,
                Err(error) => {
                    log::warn!("avatars: card read failed: {error}");
                    continue;
                }
            };
            let Some(media) = photo_resource(&card) else {
                continue;
            };
            if let Some(state) = self.photo_from_cache(&source.account, &card, &media).await {
                return state;
            }
            if let Some(state) = self
                .photo_from_provider(&source.account, &card, &media)
                .await
            {
                return state;
            }
        }
        PhotoState::None
    }

    /// The cached photo for one card resource, or `None` when the cache cannot answer.
    async fn photo_from_cache(
        &self,
        account: &engine_api::AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> Option<PhotoState> {
        match self.engine.cached_contact_photo(account, card, media).await {
            Ok(Some(file)) => Some(usable(&file.path)),
            Ok(None) => None,
            Err(error) => {
                log::warn!("avatars: cached photo read failed: {error}");
                None
            }
        }
    }

    /// Fetches one card resource's photo through the account's contacts adapter.
    async fn photo_from_provider(
        &self,
        account: &engine_api::AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> Option<PhotoState> {
        let accounts = self.accounts.read().await;
        let provider = accounts
            .iter()
            .find(|candidate| &candidate.id == account)?
            .contact_providers
            .first()?;
        let fetched = self
            .engine
            .contact_photo(provider, account, card, media)
            .await;
        match fetched {
            Ok(Some(file)) => Some(usable(&file.path)),
            // The source has no photo for this person: an answer, and one the engine has
            // already remembered so a later pass does not re-ask its provider either.
            Ok(None) => Some(PhotoState::None),
            Err(error) => {
                log::warn!("avatars: photo fetch failed: {error}");
                None
            }
        }
    }

    /// Drops everything known about sender photos, so the next rebuild asks again.
    ///
    /// Called when contacts sync, because that replaces the people index every answer was
    /// derived from. Clearing the *positive* entries too is deliberate and nearly free: a
    /// re-resolve reads the engine's own cache, which needs no provider and no network, and
    /// it is what picks up a photo that changed on the server.
    pub(crate) fn forget_sender_photos(&self) {
        self.avatar_photos
            .lock()
            .expect("avatar mutex poisoned")
            .clear();
    }

    /// Marks a pass as running, or returns `None` when one already is.
    fn begin_avatar_pass(&self) -> Option<AvatarPassGuard<'_>> {
        let mut running = self
            .avatar_pass_running
            .lock()
            .expect("avatar mutex poisoned");
        if *running {
            return None;
        }
        *running = true;
        Some(AvatarPassGuard {
            flag: &self.avatar_pass_running,
        })
    }
}

/// Clears the in-flight mark on every exit path, so an error can never leave the pass
/// permanently "already running".
struct AvatarPassGuard<'a> {
    flag: &'a std::sync::Mutex<bool>,
}

impl Drop for AvatarPassGuard<'_> {
    fn drop(&mut self) {
        *self.flag.lock().expect("avatar mutex poisoned") = false;
    }
}

/// The card's photo resource, if it advertises one.
///
/// A card may carry several media (a `PHOTO` and a `LOGO`), and only the photo is a person.
/// A resource with no `kind` is taken as the photo: CardDAV's `PHOTO` property is normalized
/// with `kind: "photo"`, but a source that states nothing is far more likely to mean its one
/// image than a logo.
fn photo_resource(card: &ContactCard) -> Option<ContactResource> {
    card.media
        .values()
        .map(|property| &property.value)
        .find(|resource| {
            resource
                .kind
                .as_deref()
                .is_none_or(|kind| kind.eq_ignore_ascii_case("photo"))
        })
        .cloned()
}

/// Decides whether a file the engine cached is something a client may be handed.
///
/// **The provider's own `media_type` is not the check.** It is remote content describing
/// itself, and a client that trusted it would decode whatever actually arrived. So the bytes
/// are sniffed, and only the four raster formats every platform decodes natively are
/// accepted.
///
/// **SVG can never pass**, by construction: it has no magic number and is script-capable, and
/// nothing in `rendering-security.md` permits it near a client surface.
fn usable(path: &std::path::Path) -> PhotoState {
    let Ok(metadata) = std::fs::metadata(path) else {
        return PhotoState::None;
    };
    if metadata.len() > MAX_PHOTO_BYTES {
        log::info!(
            "avatars: photo of {} bytes refused as oversized",
            metadata.len()
        );
        return PhotoState::None;
    }
    let Ok(head) = read_head(path) else {
        return PhotoState::None;
    };
    if is_raster(&head) {
        PhotoState::File(path.display().to_string())
    } else {
        log::info!("avatars: photo refused: not a raster image");
        PhotoState::None
    }
}

/// Reads just enough of a file to identify its format.
fn read_head(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut head = vec![0u8; 16];
    let read = file.read(&mut head)?;
    head.truncate(read);
    Ok(head)
}

/// PNG, JPEG, GIF or WebP by magic number: the one table, shared with the composer's dropped
/// pictures ([`crate::composer_image::raster_media_type`]), so the two surfaces cannot come to
/// disagree about what a client may be handed.
fn is_raster(head: &[u8]) -> bool {
    crate::composer_image::raster_media_type(head).is_some()
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
