//! Connecting the engine providers a JMAP account syncs through.
//!
//! One [`JmapProvider`] covers the **whole account**; its email scope is account-wide
//! (`JmapType { account, Email }`) and each message carries its `mailboxIds` membership, so
//! unlike IMAP/Graph there are no per-role folder providers to bind, and an on-demand folder
//! open reconnects that same account-wide provider. Split from the module root to keep both
//! files under the 500-line cap.
//!
//! # Two authentication paths, one provider shape
//!
//! A **stored-secret** account (a password or API token the user pasted) connects a plain
//! [`JmapProvider`] with those credentials, nothing expires, so nothing needs wrapping.
//!
//! An **OAuth** account cannot do that: its access token dies in about an hour while the app
//! session runs for days, so it connects the same provider and then wraps it in a
//! [`RefreshingJmapProvider`], which re-mints the token and rebuilds the delegate whenever it
//! changes. Both return `Box<dyn Provider>`, so every caller above this layer is unaware of
//! the difference.

use std::sync::Arc;

use engine_core::{ids::AddressBookId, sync::SyncUpdate};
use engine_provider::{ContactSourceSync, ContactsProvider, Provider};
use provider_jmap::{Credentials, JmapConfig, JmapProvider};

use super::{JmapAccountConfig, refreshing::RefreshingJmapProvider};
use crate::{
    AccountError, GraphTokenSource, connect_log::connect_logger, throttle::account_retry,
    tls::account_tls,
};

/// The credentials to connect a JMAP account with: either the secret stored in its config, or
/// a live access token minted from its OAuth grant.
///
/// Passed in rather than derived from the config because an OAuth account's token source is
/// **shared**; one refresh serves the account's mail and calendar providers alike, and a
/// rotated refresh token reaches the host's keystore through the source's sink. Building one
/// per connect would defeat both.
pub(crate) type JmapTokens<'a> = Option<&'a Arc<GraphTokenSource>>;

/// Connects the JMAP provider a JMAP account syncs through: **one** [`JmapProvider`]
/// covering the whole account (mailboxes + all folders' email in one account-wide
/// scope), returned boxed for the app to sync. The JMAP parallel of
/// [`connect_mail_providers`](crate::connect_mail_providers), but a single provider,
/// since JMAP's email scope is account-wide rather than per-folder.
///
/// `tokens` carries the account's shared OAuth token source, or `None` for a stored-secret
/// account.
///
/// Sync-depth is applied per sync by the app's [`engine_api::StreamTuning`], so this
/// construction path does not bake a date window into the provider.
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails, or
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to.
pub async fn connect_jmap_mail_providers(
    config: &JmapAccountConfig,
    tokens: JmapTokens<'_>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    Ok(vec![connect_one(config, tokens, None).await?])
}

/// Connects the JMAP contacts provider an account syncs its address books through.
///
/// **One** provider, unlike the CardDAV path's one-per-book fan-out: a JMAP adapter is
/// account-global; it serves every address book on the account, and its bound book only
/// decides where a *write* lands: so the engine's combined `sync_contacts` drives the whole
/// account through this single connection.
///
/// Returns an empty vector when the session does not advertise contact support, so a
/// mail-only JMAP server yields no contacts provider rather than one that fails every pass.
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails, or
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to.
pub async fn connect_jmap_contact_providers(
    config: &JmapAccountConfig,
    tokens: JmapTokens<'_>,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    let discovery = connect_one(config, tokens, None).await?;
    if !discovery.connection_info().capabilities.contacts() {
        log::info!("jmap: session advertises no contacts support; skipping address books");
        return Ok(Vec::new());
    }
    // Reconnected bound to a book, because a JMAP adapter advertises **no write destination
    // until it has one**: reading works either way, so an unbound provider looks exactly like
    // a server that refuses writes, and the client would hide its "new contact" button for a
    // server that would have accepted one. A second connect is a session GET, the same
    // throwaway discovery the CardDAV path already does.
    let book = default_address_book(discovery.as_ref()).await;
    if book.is_none() {
        log::info!("jmap: no address book to write into; contacts will be read-only");
    }
    Ok(vec![connect_one(config, tokens, book).await?])
}

/// The account's default **writable** address book, else the first writable one it lists, else
/// `None`. A book the account may only read is no write destination.
///
/// Never an error: an account whose books cannot be listed still *reads* contacts, and losing
/// that over a failed write-destination lookup would be a worse outcome than an account whose
/// contacts are read-only for this session.
async fn default_address_book(provider: &dyn ContactsProvider) -> Option<AddressBookId> {
    // Any id scopes the listing; it is not account-scoped by this value. The same throwaway
    // the CardDAV discovery uses.
    let account = engine_core::ids::AccountId::try_from("jmap-discovery").ok()?;
    let books = match provider.sync_address_books(&account, None).await {
        Ok(ContactSourceSync::Available { sync, .. }) => match sync.update {
            SyncUpdate::Snapshot { objects, .. } => objects,
            SyncUpdate::Delta { changed, .. } => changed,
        },
        Ok(ContactSourceSync::Unavailable(unavailable)) => {
            log::info!(
                "jmap: address-book listing unavailable: {}",
                unavailable.reason
            );
            return None;
        }
        Err(error) => {
            log::warn!("jmap: address-book listing failed: {error}");
            return None;
        }
    };
    books
        .iter()
        .find(|book| book.is_default && book.is_writable)
        .or_else(|| books.iter().find(|book| book.is_writable))
        .map(|book| book.id.clone())
}

/// Connects an on-demand JMAP provider for a folder of a JMAP account. JMAP's one
/// provider syncs the account-wide email scope, so the specific folder is irrelevant
/// : the reconnected provider covers it (a cheap delta after the boot sync). The JMAP
/// parallel of [`connect_imap_mailbox`](crate::connect_imap_mailbox) /
/// [`connect_graph_folder`](crate::connect_graph_folder).
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails, or
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to.
pub async fn connect_jmap_folder(
    config: &JmapAccountConfig,
    tokens: JmapTokens<'_>,
) -> Result<Box<dyn Provider>, AccountError> {
    // Upcast: `ContactsProvider: Provider`, so the contacts-shaped box this connect returns
    // is a mail provider too. Explicit because a coercion site in tail position is not one
    // Rust infers.
    Ok(connect_one(config, tokens, None).await?)
}

/// Connects the JMAP calendar provider a JMAP account syncs its agenda through:
/// **one** [`JmapProvider`] covering the account-wide calendar scope
/// (`Calendar`/`CalendarEvent`). A JMAP provider serves mail **and** calendar, so
/// this is a second connection to the same server (stateless HTTP; cheap). The
/// caller connects this only when the session advertises calendar support (see
/// [`Provider::connection_info`]); a mail-only JMAP server yields no calendar provider.
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails, or
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to.
pub async fn connect_jmap_calendar_providers(
    config: &JmapAccountConfig,
    tokens: JmapTokens<'_>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    Ok(vec![connect_one(config, tokens, None).await?])
}

/// Connects a single JMAP provider from `config`, boxed behind the neutral
/// [`ContactsProvider`] contract; self-refreshing when the account is OAuth, plain otherwise.
///
/// Boxed as `dyn ContactsProvider` rather than `dyn Provider` because both concrete types
/// implement it and `ContactsProvider: Provider`, so the mail and calendar callers upcast to
/// `Box<dyn Provider>` for free. The alternative: a second near-identical connect that
/// differed only in its box, is exactly the pair that drifts.
///
/// `contact_book` binds the provider's contact **write** destination. Only the contacts
/// connect passes one; the mail and calendar paths never write a card, and a destination they
/// carried would be one more thing to keep in step for nothing.
async fn connect_one(
    config: &JmapAccountConfig,
    tokens: JmapTokens<'_>,
    contact_book: Option<AddressBookId>,
) -> Result<Box<dyn ContactsProvider>, AccountError> {
    let tls = account_tls()?;
    let Some(tokens) = tokens.filter(|_| config.is_oauth()) else {
        let provider = JmapProvider::connect(config.engine_config(tls))
            .await
            .map_err(|err| AccountError::from_jmap_connect(&err))?;
        return Ok(Box::new(bind_book(provider, contact_book)));
    };

    // Mint a token and connect once here rather than lazily: it proves the grant still works
    // (a revoked one surfaces as a connect failure now, not on the first background sync) and
    // it reads the server's real capabilities from the session, which the wrapper must be
    // able to report synchronously without ever having to guess.
    let access_token = tokens.access_token().await?;
    let engine_config = JmapConfig::new(
        config.base_url.clone(),
        Credentials::bearer(access_token.clone()),
    )
    .with_tls(tls.clone())
    .with_retry(account_retry())
    .with_connect_observer(connect_logger("jmap"));
    let provider = JmapProvider::connect(engine_config)
        .await
        .map_err(|err| AccountError::from_jmap_connect(&err))?;
    let capabilities = provider.connection_info().capabilities;
    Ok(Box::new(RefreshingJmapProvider::new(
        config.base_url.clone(),
        Arc::clone(tokens),
        tls,
        access_token,
        bind_book(provider, contact_book.clone()),
        capabilities,
        // The wrapper rebuilds its delegate on every token change, so it has to be told the
        // book too: a rebuilt delegate that forgot it would silently stop advertising a write
        // destination an hour into the session.
        contact_book,
    )))
}

fn bind_book(provider: JmapProvider, book: Option<AddressBookId>) -> JmapProvider {
    match book {
        Some(book) => provider.with_contact_address_book(book),
        None => provider,
    }
}
