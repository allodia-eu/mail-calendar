//! Finding one person's photo: their source cards, the engine's cache, the account's contacts
//! provider, and the check that a file is something a client may be handed.

use engine_api::{ContactCard, ContactResource, Provider};

use super::PhotoState;
use crate::App;

/// The largest photo accepted, in bytes.
///
/// A contact photo is drawn at avatar size. Anything past this is a provider handing us a
/// full-resolution image, and decoding it per row costs far more than the monogram it would
/// replace.
const MAX_PHOTO_BYTES: u64 = 2 * 1024 * 1024;

impl<P: Provider> App<P> {
    /// Walks one person's source cards for a usable photo, or `None` when no source answered
    /// and at least one could not be asked.
    ///
    /// Cache first and without a provider, because most calls are re-reads after a restart:
    /// the engine's cache survives, this in-memory map does not. Only a genuine miss needs a
    /// connected adapter. A source that could not be asked (a failed read, a provider that is
    /// not connected, a failed fetch) is not a source that has no photo: the walk goes on to the
    /// next, and if none answers the caller keeps what it knew (`unanswered`).
    pub(super) async fn photo_for_person(&self, person: &engine_api::Person) -> Option<PhotoState> {
        let mut unasked = false;
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
                    unasked = true;
                    continue;
                }
            };
            let Some(media) = photo_resource(&card) else {
                continue;
            };
            if let Some(state) = self.photo_from_cache(&source.account, &card, &media).await {
                return Some(state);
            }
            match self
                .photo_from_provider(&source.account, &card, &media)
                .await
            {
                Some(state) => return Some(state),
                None => unasked = true,
            }
        }
        (!unasked).then_some(PhotoState::None)
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

    /// Fetches one card resource's photo through the adapter whose source holds the card, or
    /// `None` when it cannot be asked: no connected adapter holds it, or the fetch failed. An
    /// account connects one adapter per source and each knows only its own ids.
    async fn photo_from_provider(
        &self,
        account: &engine_api::AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> Option<PhotoState> {
        let accounts = self.accounts.read().await;
        let providers = &accounts
            .iter()
            .find(|candidate| &candidate.id == account)?
            .contact_providers;
        let provider = match self.engine.contact_source(providers, account, &card.id).await {
            Ok(provider) => provider?,
            Err(error) => {
                log::warn!("avatars: card source read failed: {error}");
                return None;
            }
        };
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
}

/// The card's photo resource, if it advertises one.
///
/// A card may carry several media (a `PHOTO` and a `LOGO`), and only the photo is a person.
/// A resource with no `kind` is taken as the photo: CardDAV's `PHOTO` property is normalised
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
pub(super) fn usable(path: &std::path::Path) -> PhotoState {
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
pub(super) fn is_raster(head: &[u8]) -> bool {
    crate::composer_image::raster_media_type(head).is_some()
}
