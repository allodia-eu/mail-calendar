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

use engine_provider::{ContactsProvider, Provider};
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
    Ok(vec![connect_one(config, tokens).await?])
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
    let provider = connect_one(config, tokens).await?;
    if provider.connection_info().capabilities.contacts() {
        Ok(vec![provider])
    } else {
        log::info!("jmap: session advertises no contacts support; skipping address books");
        Ok(Vec::new())
    }
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
    Ok(connect_one(config, tokens).await?)
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
    Ok(vec![connect_one(config, tokens).await?])
}

/// Connects a single JMAP provider from `config`, boxed behind the neutral
/// [`ContactsProvider`] contract; self-refreshing when the account is OAuth, plain otherwise.
///
/// Boxed as `dyn ContactsProvider` rather than `dyn Provider` because both concrete types
/// implement it and `ContactsProvider: Provider`, so the mail and calendar callers upcast to
/// `Box<dyn Provider>` for free. The alternative: a second near-identical connect that
/// differed only in its box, is exactly the pair that drifts.
async fn connect_one(
    config: &JmapAccountConfig,
    tokens: JmapTokens<'_>,
) -> Result<Box<dyn ContactsProvider>, AccountError> {
    let tls = account_tls()?;
    let Some(tokens) = tokens.filter(|_| config.is_oauth()) else {
        let provider = JmapProvider::connect(config.engine_config(tls))
            .await
            .map_err(|err| AccountError::from_jmap_connect(&err))?;
        return Ok(Box::new(provider));
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
        provider,
        capabilities,
    )))
}
