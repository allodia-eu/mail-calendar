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
//! **Where the CardDAV endpoint comes from.** The account's `[carddav]` section when it has one,
//! and otherwise its `[caldav]` origin and credentials, letting `.well-known/carddav` find the
//! address-book home from there. Virtually every server that speaks CalDAV for an account speaks
//! CardDAV for it at the same origin with the same login, so the section exists for the server
//! that splits them and for an account used for its contacts alone.

use engine_api::AccountId;
use engine_core::sync::SyncUpdate;
use engine_provider::{ContactSourceSync, ContactsProvider, Provider};
use futures::StreamExt;
use provider_caldav::{CardDavConfig, CardDavProvider};

use crate::{
    AccountConfig, AccountError, DavConnected, setup::normalize_caldav_base_url,
    throttle::ungated_retry, tls::account_tls,
};

/// The account id used only to scope the discovery listing. The real per-account id is
/// applied by the app when it syncs; discovery just needs *an* id to pass, and the
/// listing it returns is not account-scoped by this value.
const DISCOVERY_ACCOUNT: &str = "carddav-discovery";

/// How many address books connect at once. No DAV server here states a ceiling, so this only
/// keeps an account with many shared books from opening a connection for each at the same time.
const BOOKS_AT_ONCE: usize = 4;

/// Connects one CardDAV adapter per address book the account exposes.
///
/// Discovers the books over one connection, rebinds it to the first and connects the rest side by
/// side, a few at a time. A book
/// that cannot be connected is **skipped with a warning rather than failing the account**:
/// one unreadable shared book must not cost the user their personal contacts. An account
/// whose server advertises no contact support, or which exposes no books at all, yields an
/// empty vector: not an error, since "this account has no address books" is an ordinary
/// state and the caller has nothing to tell the user about it.
///
/// # Errors
///
/// Returns [`AccountError`] if the config has neither a `[carddav]` nor a `[caldav]` section,
/// the shared TLS policy
/// cannot be built, or the *discovery* connection itself fails.
pub async fn connect_carddav_contact_providers(
    account: &AccountConfig,
    tokens: crate::ImapTokens<'_>,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    connect_carddav_learning(account, tokens)
        .await
        .map(|connected| connected.providers)
}

/// [`connect_carddav_contact_providers`], reporting the principal the server named. It starts at
/// the principal the last connect stored ([`DavStarts::contacts`](crate::DavStarts::contacts))
/// when there is one, and from the configured server when there is none or the server says it is
/// gone. A configured path that is gone falls back to the server's well-known, which is also where
/// contacts on the calendar's endpoint start.
///
/// # Errors
///
/// As [`connect_carddav_contact_providers`].
pub async fn connect_carddav_learning(
    account: &AccountConfig,
    tokens: crate::ImapTokens<'_>,
) -> Result<DavConnected<Vec<Box<dyn ContactsProvider>>>, AccountError> {
    if let Some(start) = account.dav_starts.contacts.as_deref() {
        match connect_carddav_at(account, tokens, Some(start)).await {
            Err(err) if err.is_gone() => {
                log::info!(
                    "carddav: the stored principal is gone; starting from the configured server"
                );
            }
            connected => return connected.map(|connected| connected.started_at(start)),
        }
    }
    // Contacts on the calendar's endpoint look for the address book from its origin: the
    // calendar's path is where a CalDAV server keeps calendars.
    let Some(carddav) = &account.carddav else {
        return connect_carddav_at(account, tokens, Some(WELL_KNOWN)).await;
    };
    match connect_carddav_at(account, tokens, None).await {
        Err(err) if err.is_gone() && crate::starts_at_a_path(&carddav.base_url) => {
            log::info!("carddav: the configured path is gone; starting from the well-known");
            connect_carddav_at(account, tokens, Some(WELL_KNOWN)).await
        }
        connected => connected,
    }
}

/// Where a CardDAV server is found from its origin (RFC 6764 §5).
const WELL_KNOWN: &str = "/.well-known/carddav";

/// One CardDAV connect, starting at `start` when given and at the configured server otherwise.
async fn connect_carddav_at(
    account: &AccountConfig,
    tokens: crate::ImapTokens<'_>,
    start: Option<&str>,
) -> Result<DavConnected<Vec<Box<dyn ContactsProvider>>>, AccountError> {
    // The account's own `[carddav]` endpoint, or the calendar's when it has none; Basic from
    // the stored password, or the account grant's access token where it may go.
    let endpoint = account.carddav_endpoint().ok_or(AccountError::NoCalDav)?;
    let auth = crate::config_dav::dav_credentials(
        account,
        endpoint,
        mailcal_oauth::DavUse::Contacts,
        tokens,
    )
    .await?;
    let tls = account_tls(account)?;
    let mut config = CardDavConfig::new(
        // Tolerate a stored bare host the same way `connect_caldav` does, so an account
        // set up before scheme normalisation still connects.
        normalize_caldav_base_url(endpoint.base_url),
        auth.credentials,
        crate::user_agent(),
    );
    if let Some(hosts) = auth.hosts {
        config = config.with_credential_hosts(hosts);
    }
    if let Some(start) = start {
        config = config.with_discovery_path(start);
    }
    let config = config
    // A clone shares the record of what it refused, so a refusal reads back from `tls`.
    .with_tls(tls.clone())
    // Ungated: no CalDAV/CardDAV adapter states a concurrency ceiling, because no RFC gives
    // one and no server here has been measured (`docs/agent-guidance/http-throttling.md`).
    // A gate with nothing to narrow it bounds nothing, so wiring one would be ceremony.
    // When a DAV ceiling is measured, this needs the account id threading through.
    .with_retry(ungated_retry())
    .with_connect_observer(crate::connect_log::connect_logger("carddav"));

    let discovery = CardDavProvider::connect(config.clone())
        .await
        .map_err(|err| {
            AccountError::from_first_dav_connect(err, AccountError::CardDav).over_tls(&tls)
        })?;
    let principal = discovery.principal_url().map(str::to_owned);
    let none = |principal| DavConnected {
        providers: Vec::new(),
        principal,
    };
    // The server says whether it has address books (the `addressbook` class, RFC 6352 §6.1): an
    // account whose CalDAV origin serves no CardDAV reports no contacts capability, and syncing it
    // would fail once per pass forever.
    if !discovery.connection_info().capabilities.contacts() {
        log::info!("carddav: server advertises no contacts support; skipping address books");
        return Ok(none(principal));
    }

    let books = discover_address_books(&discovery).await?;
    log::info!("carddav: discovered {} address book(s)", books.len());
    let mut books = books.into_iter();
    let Some(first) = books.next() else {
        return Ok(none(principal));
    };

    // The discovery connection already holds the home, so the first book rebinds it. Every other
    // book needs a connection of its own, and each connect discovers the home again, so they run
    // side by side: one after another, a slow server's round trips add up to what the account's
    // connect waits for.
    let first = discovery.rebind(&first).map_err(|error| (first, error));
    let rest = futures::stream::iter(books)
        .map(|book| {
            let config = config.clone().with_address_book(book.clone());
            async move {
                CardDavProvider::connect(config)
                    .await
                    .map_err(|error| (book, error))
            }
        })
        .buffered(BOOKS_AT_ONCE)
        .collect::<Vec<_>>()
        .await;
    let providers = std::iter::once(first)
        .chain(rest)
        .filter_map(|bound| match bound {
            Ok(provider) => Some(Box::new(provider) as Box<dyn ContactsProvider>),
            // Skipped, not fatal; see the doc comment. Logged with the book id only: an
            // address-book id is not contact content (docs/logging.md).
            Err((book, error)) => {
                log::warn!("carddav: skipping address book {book}: {error}");
                None
            }
        })
        .collect();
    Ok(DavConnected {
        providers,
        principal,
    })
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
