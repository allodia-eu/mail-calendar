//! Connecting the engine providers a JMAP account syncs through.
//!
//! One [`JmapProvider`] covers the **whole account**; its email scope is account-wide
//! (`JmapType { account, Email }`) and each message carries its `mailboxIds` membership, so
//! unlike IMAP/Graph there are no per-folder providers to bind, and an on-demand folder
//! open reconnects that same account-wide provider. Split from the module root to keep both
//! files under the 500-line cap.
//!
//! # One provider, connected again only after a dead connection
//!
//! Every account connects one [`JmapProvider`] over its credential source
//! (`JmapAccountConfig::credential_source`), which the engine asks on every request: a stored
//! secret answers the same each time, and an OAuth account's token source answers its current
//! access token and, after a `401` that refuses it, a replacement through its single-flight
//! refresh. The engine judges the `401`: a bare one, or one that challenges for Bearer, refuses
//! the token; one that offers only Basic, which a token cannot be framed as, is final. So the
//! provider outlives every token. It is connected again, with the same bindings, only after a
//! request found its connection dead ([`ReconnectingJmapProvider`]).

use engine_core::{
    ids::{AddressBookId, SharedAccountId},
    sync::SyncUpdate,
};
use engine_provider::{ContactSourceSync, ContactsProvider, Provider, ProviderError};
use provider_jmap::{JmapConfig, JmapProvider};

use super::{JmapAccountConfig, reconnecting::ReconnectingJmapProvider};
use crate::{AccountError, AccountTokens, error::SignsIn, tls::tls_with};

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
/// `shared` binds the provider to a mailbox shared with the account rather than its own.
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails,
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to, or [`AccountError::MissingCredential`] if
/// the account has none to present: an OAuth account given no token source, or one that stores
/// neither a secret nor a grant.
pub async fn connect_jmap_mail_providers(
    config: &JmapAccountConfig,
    tokens: AccountTokens<'_>,
    shared: Option<&SharedAccountId>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    Ok(vec![connect_one(config, tokens, None, shared).await?])
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
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails,
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to, or [`AccountError::MissingCredential`] if
/// the account has none to present: an OAuth account given no token source, or one that stores
/// neither a secret nor a grant.
pub async fn connect_jmap_contact_providers(
    config: &JmapAccountConfig,
    tokens: AccountTokens<'_>,
) -> Result<Vec<Box<dyn ContactsProvider>>, AccountError> {
    let discovery = connect_one(config, tokens, None, None).await?;
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
    Ok(vec![connect_one(config, tokens, book, None).await?])
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
/// [`connect_graph_folder`](crate::connect_graph_folder). `shared` names the shared mailbox the
/// folder belongs to.
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails,
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to, or [`AccountError::MissingCredential`] if
/// the account has none to present: an OAuth account given no token source, or one that stores
/// neither a secret nor a grant.
pub async fn connect_jmap_folder(
    config: &JmapAccountConfig,
    tokens: AccountTokens<'_>,
    shared: Option<&SharedAccountId>,
) -> Result<Box<dyn Provider>, AccountError> {
    // Upcast: `ContactsProvider: Provider`, so the contacts-shaped box this connect returns
    // is a mail provider too. Explicit because a coercion site in tail position is not one
    // Rust infers.
    Ok(connect_one(config, tokens, None, shared).await?)
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
/// Returns [`AccountError::Jmap`] if the connection or session discovery fails,
/// [`AccountError::SigninRejected`] if the server refuses the account's credential: a dead OAuth
/// grant, or a password/API token it answers `401` to, or [`AccountError::MissingCredential`] if
/// the account has none to present: an OAuth account given no token source, or one that stores
/// neither a secret nor a grant.
///
/// `shared` binds it to the calendars of a store shared with the account rather than its own.
pub async fn connect_jmap_calendar_provider(
    config: &JmapAccountConfig,
    tokens: AccountTokens<'_>,
    shared: Option<&SharedAccountId>,
) -> Result<Box<dyn Provider>, AccountError> {
    Ok(connect_one(config, tokens, None, shared).await?)
}

/// Connects a single JMAP provider from `config`, boxed behind the neutral
/// [`ContactsProvider`] contract.
///
/// Boxed as `dyn ContactsProvider` rather than `dyn Provider` because `ContactsProvider:
/// Provider`, so the mail and calendar callers upcast to `Box<dyn Provider>` for free. The
/// alternative: a second near-identical connect that differed only in its box, is exactly the
/// pair that drifts.
///
/// `contact_book` binds the provider's contact **write** destination. Only the contacts
/// connect passes one; the mail and calendar paths never write a card, and a destination they
/// carried would be one more thing to keep in step for nothing. `shared` binds it to a mailbox
/// shared with the account rather than the account's own.
///
/// The connect discovers the session, which presents the credential: a revoked grant or a
/// refused secret fails here as [`AccountError::SigninRejected`] rather than at the first
/// background sync, and the provider reports the server's capabilities from that session.
async fn connect_one(
    config: &JmapAccountConfig,
    tokens: AccountTokens<'_>,
    contact_book: Option<AddressBookId>,
    shared: Option<&SharedAccountId>,
) -> Result<Box<dyn ContactsProvider>, AccountError> {
    let account = config
        .account_id()
        .map_err(|err| AccountError::Jmap(err.to_string()))?;
    let source = config.credential_source(&account, tokens)?;
    let engine_config = bind_shared(
        config.engine_config(source, tls_with(&[])?, &account),
        shared,
    );
    let provider = JmapProvider::connect(engine_config.clone())
        .await
        .map_err(|err| {
            AccountError::from_jmap_connect(&err, SignsIn::grant_if(config.is_oauth()))
        })?;
    let connected = bind_book(provider, contact_book.clone());
    let redial = Box::new(move || {
        let (config, book) = (engine_config.clone(), contact_book.clone());
        Box::pin(async move {
            let provider = JmapProvider::connect(config)
                .await
                .map_err(ProviderError::from)?;
            Ok(bind_book(provider, book))
        }) as _
    });
    Ok(Box::new(ReconnectingJmapProvider::adopt(connected, redial)))
}

/// `config` bound to the mailbox `shared` names, or to the account's own when it is `None`.
fn bind_shared(config: JmapConfig, shared: Option<&SharedAccountId>) -> JmapConfig {
    match shared {
        Some(handle) => config.with_shared_account(handle.clone()),
        None => config,
    }
}

fn bind_book(provider: JmapProvider, book: Option<AddressBookId>) -> JmapProvider {
    match book {
        Some(book) => provider.with_contact_address_book(book),
        None => provider,
    }
}

#[cfg(test)]
#[path = "connect_tests.rs"]
mod tests;
