//! Connecting the contact-source adapters an account syncs its address books through.
//!
//! Two shapes, because the two protocols differ in where a "source" lives:
//!
//! - **CardDAV adapters are source-bound.** One [`CardDavProvider`] speaks for exactly one
//!   address-book collection, so an account with a personal book and a shared team book needs *two*
//!   address books, which are discovered here and connected one apiece, the same fan-out the mail
//!   side does per folder.
//! - **The JMAP adapter is account-global.** One provider serves every address book on the account
//!   (its bound book only decides where a *write* lands), so the account contributes exactly one,
//!   driven through the engine's combined `sync_contacts`.
//!
//! **Where the CardDAV endpoint comes from.** There is deliberately no `[carddav]` config
//! section: contacts reuse the account's `[caldav]` origin and credentials, and let
//! `.well-known/carddav` find the address-book home from there. Virtually every server that
//! speaks CalDAV for an account speaks CardDAV for it at the same origin with the same login,
//! so a separate section would be a second thing for a user to type and get wrong. If a server
//! ever needs them split, that is the moment to add the section: not before.

use engine_api::AccountId;
use engine_core::sync::SyncUpdate;
use engine_provider::{ContactSourceSync, ContactsProvider, Provider};
use provider_caldav::{CardDavConfig, CardDavProvider, Credentials};

use crate::{
    AccountConfig, AccountError, setup::normalize_caldav_base_url, throttle::account_retry,
    tls::account_tls,
};

/// The account id used only to scope the discovery listing. The real per-account id is
/// applied by the app when it syncs; discovery just needs *an* id to pass, and the
/// listing it returns is not account-scoped by this value.
const DISCOVERY_ACCOUNT: &str = "carddav-discovery";

/// Connects one CardDAV adapter per address book the account exposes.
///
/// Discovers the books over a throwaway connection, then binds a provider to each. A book
/// that cannot be connected is **skipped with a warning rather than failing the account**:
/// one unreadable shared book must not cost the user their personal contacts. An account
/// whose server advertises no contact support, or which exposes no books at all, yields an
/// empty vector: not an error, since "this account has no address books" is an ordinary
/// state and the caller has nothing to tell the user about it.
///
/// # Errors
///
/// Returns [`AccountError`] if the config has no `[caldav]` section, the shared TLS policy
/// cannot be built, or the *discovery* connection itself fails.
pub async fn connect_carddav_contact_providers(
    account: &AccountConfig,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    let caldav = account.caldav.as_ref().ok_or(AccountError::NoCalDav)?;
    let tls = account_tls()?;
    let config = CardDavConfig::new(
        // Tolerate a stored bare host the same way `connect_caldav` does, so an account
        // set up before scheme normalisation still connects.
        normalize_caldav_base_url(&caldav.base_url),
        Credentials::Basic {
            username: caldav.username.clone(),
            password: caldav.password.expose().to_owned(),
        },
    )
    .with_tls(tls)
    .with_retry(account_retry());

    let discovery = CardDavProvider::connect(config.clone()).await?;
    // Ask the server before assuming: an account whose CalDAV origin serves no CardDAV
    // reports no contacts capability, and syncing it would fail once per pass forever.
    if !discovery.connection_info().capabilities.contacts() {
        log::info!("carddav: server advertises no contacts support; skipping address books");
        return Ok(Vec::new());
    }

    let books = discover_address_books(&discovery).await?;
    log::info!("carddav: discovered {} address book(s)", books.len());

    let mut providers: Vec<Box<dyn ContactsProvider>> = Vec::new();
    for book in books {
        match CardDavProvider::connect(config.clone().with_address_book(book.clone())).await {
            Ok(provider) => providers.push(Box::new(provider)),
            // Skipped, not fatal; see the doc comment. Logged with the book id only: an
            // address-book id is not contact content (docs/logging.md).
            Err(error) => log::warn!("carddav: skipping address book {book}: {error}"),
        }
    }
    Ok(providers)
}

/// Lists the address-book ids the connected adapter can see.
async fn discover_address_books(provider: &CardDavProvider) -> Result<Vec<String>, AccountError> {
    let account = AccountId::try_from(DISCOVERY_ACCOUNT)
        .map_err(|error| AccountError::CalDavDiscovery(error.to_string()))?;
    let listing = provider
        .sync_address_books(&account, None)
        .await
        .map_err(|error| AccountError::CalDavDiscovery(error.to_string()))?;
    let sync = match listing {
        ContactSourceSync::Available { sync, .. } => sync,
        // A source that declines to be read is not an error for the *account*; it is one
        // source being unavailable while its siblings may still work.
        ContactSourceSync::Unavailable(unavailable) => {
            log::info!(
                "carddav: address-book listing unavailable: {}",
                unavailable.reason,
            );
            return Ok(Vec::new());
        }
    };
    Ok(match sync.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    }
    .into_iter()
    .map(|book| book.id.as_str().to_owned())
    .collect())
}
