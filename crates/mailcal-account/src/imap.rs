//! Connecting an account's IMAP providers: resolving the credential for a dial, and the
//! self-healing wrappers the app syncs through.
//!
//! Split from the crate root, which had grown past the size limit, and along the seam that
//! matters: everything here is about *how a session comes to exist* for an IMAP account,
//! including the one thing that made this non-trivial: an account may authenticate with a
//! stored password or with an access token that expires within the hour, and only the second
//! makes a credential something to resolve per dial rather than read once.

use std::sync::Arc;

use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId},
    mail::MailboxRole,
    sync::SyncUpdate,
};
use engine_provider::{Provider, ProviderError, Watch};
use provider_imap::{Credentials, DEFAULT_IDLE_KEEPALIVE, ImapConfig, ImapProvider, ImapWatcher};

use crate::{
    AccountConfig, AccountError, GraphTokenSource,
    reconnect::{AuthRenewal, ReconnectingImapProvider, Redial},
    tls::account_tls,
};

/// The token source an **OAuth** IMAP account dials with; `None` for a password account.
///
/// Deliberately not folded into [`AccountConfig`]. The source holds live process state (a
/// cached access token, and the refresh single-flight every provider of this account shares)
/// while the config is inert data a host reads out of its keystore. One is rebuilt each
/// launch; the other is not.
pub type ImapTokens<'a> = Option<&'a Arc<GraphTokenSource>>;

/// The non-INBOX folder roles the app eagerly binds a provider to at startup, so their
/// messages sync and render up front (Sent threads a reply with its original; Trash shows
/// deleted mail; Drafts / Archive / Junk are the other folders a user navigates first).
/// Folder names are server-specific, so each is resolved by its SPECIAL-USE role. Any
/// **other** folder (a server that doesn't tag Archive, or a custom folder) syncs **on
/// demand** when the user opens it, via the host's `MailboxConnector` +
/// [`connect_imap_mailbox`]: so no folder is permanently empty.
const SYNCED_ROLES: &[MailboxRole] = &[
    MailboxRole::Sent,
    MailboxRole::Drafts,
    MailboxRole::Trash,
    MailboxRole::Archive,
    MailboxRole::Junk,
];

/// Applies the optional sync-depth cutoff to an IMAP config: a `Some(date)` bounds mail
/// sync to messages delivered on or after it (`ImapConfig::with_since`); `None` syncs the
/// whole mailbox. One place so every connect path windows consistently.
fn windowed(config: ImapConfig, since: Option<time::Date>) -> ImapConfig {
    match since {
        Some(date) => config.with_since(date),
        None => config,
    }
}

/// Resolves the credential for **one** dial: the stored password, or an access token minted
/// now from the account's grant.
///
/// Called per dial rather than once per account, because an OAuth access token outlives
/// neither the app session nor, usually, the hour: a config built once and reused would
/// authenticate for exactly as long as its first token, then fail in a way that looks like the
/// server refusing a correct credential.
///
/// # Errors
///
/// Returns [`AccountError::SigninRejected`] when the grant is revoked or expired (the refresh
/// mints nothing and the user must sign in again), a transport error from the refresh, or
/// [`AccountError::Imap`] when the account has neither credential, which a stored config
/// should make impossible and a hand-edited one does not.
pub async fn imap_credentials(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
) -> Result<Credentials, AccountError> {
    if account.is_oauth() {
        let tokens = tokens.ok_or_else(|| {
            AccountError::Jmap(
                "this account signs in with OAuth but was connected without a token source"
                    .to_owned(),
            )
        })?;
        let access_token = tokens.access_token().await?;
        return Ok(Credentials::oauth2(
            account.imap.username.clone(),
            access_token,
        ));
    }
    account.imap_password_credentials().ok_or_else(|| {
        AccountError::Jmap("this account stores neither a password nor a grant".to_owned())
    })
}

/// Builds the whole dial config for one connection: a freshly resolved credential, the
/// account's transports, and the sync-depth window.
async fn dial_config(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
    since: Option<time::Date>,
) -> Result<ImapConfig, AccountError> {
    let credentials = imap_credentials(account, tokens).await?;
    Ok(windowed(account.imap_config(credentials), since))
}

/// Builds the re-dial closure a [`ReconnectingImapProvider`] uses to rebuild a dropped IMAP
/// session: it re-resolves the credential and re-runs [`ImapProvider::connect`] with the same
/// windowed transports and bound `mailbox`, so a reconnect re-applies the sync-depth window
/// and re-selects the same folder.
///
/// The credential is resolved **inside** the closure, which is the whole point on an OAuth
/// account: the token that opened the first session has very likely expired by the time
/// anything needs a second one. The shared TLS config is captured by value and cloned per dial
/// (an `Arc` bump), keeping every reconnect on the account's selected trust policy.
fn make_imap_redial(
    account: AccountConfig,
    tokens: Option<Arc<GraphTokenSource>>,
    since: Option<time::Date>,
    mailbox: MailboxId,
    tls: engine_tls::TlsClientConfig,
) -> Redial {
    Box::new(move || {
        let account = account.clone();
        let tokens = tokens.clone();
        let mailbox = mailbox.clone();
        let tls = tls.clone();
        Box::pin(async move {
            let config = dial_config(&account, tokens.as_ref(), since)
                .await
                .map_err(|err| redial_failure(&err))?;
            ImapProvider::connect(&config, tls.connector(), mailbox)
                .await
                .map(|provider| Arc::new(provider) as Arc<dyn Provider>)
                .map_err(ProviderError::from)
        })
    })
}

/// Reports a credential that could not be resolved for a re-dial as a provider failure of the
/// right class.
///
/// The class is not cosmetic: it decides what the app does next. A revoked or expired grant is
/// [`Authentication`](FailureClass::Authentication), which becomes "sign in again"; a refresh
/// that could not reach the token endpoint is [`Retryable`](FailureClass::Retryable) and must
/// not prompt anybody, because nothing about the account has changed.
fn redial_failure(err: &AccountError) -> ProviderError {
    let class = match err {
        AccountError::SigninRejected(_) => FailureClass::Authentication,
        _ => FailureClass::Retryable,
    };
    ProviderError::new(class, err.to_string())
}

/// Whether a re-dial of this account can change the answer to an authentication failure.
///
/// It can for OAuth and cannot for a password, and the difference is not a nicety: an expired
/// access token is the ordinary mid-session event on an OAuth account and a redial fixes it,
/// while a refused password is refused again by every subsequent attempt, at a provider that
/// may well be counting them.
fn auth_renewal(account: &AccountConfig) -> AuthRenewal {
    if account.is_oauth() {
        AuthRenewal::MintsAFreshToken
    } else {
        AuthRenewal::Impossible
    }
}

/// Connects to one IMAP `mailbox` of `account` over a certificate-verifying TLS
/// connector (Mozilla roots), bounding mail sync to `since` (the sync-depth cutoff;
/// `None` for all mail), returning the provider boxed for the app to sync. Used by the
/// host's on-demand "sync the folder you open" path.
///
/// # Errors
///
/// Returns [`AccountError`] if `mailbox` is not a valid id, the credential cannot be
/// resolved, or the connection/login fails.
pub async fn connect_imap_mailbox(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
    mailbox: &str,
    since: Option<time::Date>,
) -> Result<Box<dyn Provider>, AccountError> {
    let mailbox =
        MailboxId::try_from(mailbox).map_err(|err| AccountError::Mailbox(err.to_string()))?;
    let config = dial_config(account, tokens, since).await?;
    let tls = account_tls()?;
    let provider = ImapProvider::connect(&config, tls.connector(), mailbox.clone()).await?;
    let provider: Arc<dyn Provider> = Arc::new(provider);
    let redial = make_imap_redial(
        account.clone(),
        tokens.cloned(),
        since,
        mailbox.clone(),
        tls,
    );
    Ok(Box::new(ReconnectingImapProvider::adopt(
        provider,
        mailbox,
        redial,
        auth_renewal(account),
    )))
}

/// Opens a standing IMAP `IDLE` watch on one `mailbox` of `account`, over the same
/// certificate-verifying TLS connector as the sync providers, returning it boxed behind
/// the engine's neutral [`Watch`] contract. The watch is a **separate connection** from
/// the sync provider (a connection in `IDLE` cannot `FETCH`), so the host drives this from
/// its own task and runs the mailbox's sync on the provider when it reports
/// [`WatchEvent`](engine_provider::WatchEvent)`::Changed`. No sync-depth window applies; a
/// watch carries no data, only the signal to sync (the sync itself windows). Used by the
/// host's "receive emails as they come in" (push) path; the parallel of
/// [`connect_imap_mailbox`] for watching rather than syncing.
///
/// # Errors
///
/// Returns [`AccountError`] if `mailbox` is not a valid id, the credential cannot be resolved,
/// the connection/login fails, or the server does not advertise `IDLE` (the host then falls
/// back to polling).
pub async fn connect_imap_watcher(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
    mailbox: &str,
) -> Result<Box<dyn Watch>, AccountError> {
    let mailbox =
        MailboxId::try_from(mailbox).map_err(|err| AccountError::Mailbox(err.to_string()))?;
    // A watch carries no mail, so it is never windowed: the sync it triggers applies the
    // sync-depth cutoff. The keep-alive is the engine's RFC 2177-safe default (clamped by
    // the adapter); a shorter mobile interval is a future per-platform refinement.
    let config = dial_config(account, tokens, None).await?;
    let tls = account_tls()?;
    let watcher = ImapWatcher::connect(&config, tls.connector(), mailbox, DEFAULT_IDLE_KEEPALIVE)
        .await
        .map_err(|err| AccountError::Watch(err.to_string()))?;
    Ok(Box::new(watcher))
}

/// Connects the IMAP providers the app syncs: the INBOX plus every folder carrying one
/// of the `SYNCED_ROLES` (Sent, Drafts, Trash, Archive, Junk), each resolved by its
/// role (its name is server-specific) from the account's folder list. So sent mail
/// threads with its original and the Trash/Drafts/etc. folders render their contents.
/// Returns one boxed provider per bound mailbox (just the INBOX when none of the roles
/// exist).
///
/// # Errors
///
/// Returns [`AccountError`] if the credential cannot be resolved, a connection/login fails, or
/// the folder list cannot be fetched.
pub async fn connect_mail_providers(
    account: &AccountConfig,
    tokens: ImapTokens<'_>,
    account_id: &AccountId,
    since: Option<time::Date>,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let config = dial_config(account, tokens, since).await?;
    let tls = account_tls()?;
    let inbox_id =
        MailboxId::try_from("INBOX").map_err(|err| AccountError::Mailbox(err.to_string()))?;
    // The account's first login, and the only one that can prove the stored credential wrong: a
    // refusal here has nothing to contradict it, while one in the folder loop below has the
    // success of this connect (see `from_first_imap_login`).
    let inbox = ImapProvider::connect(&config, tls.connector(), inbox_id.clone())
        .await
        .map_err(AccountError::from_first_imap_login)?;
    let inbox: Arc<dyn Provider> = Arc::new(inbox);

    // Enumerate folders to find the role mailboxes (their names vary by server).
    let listing = inbox
        .sync_mailboxes(account_id, None)
        .await
        .map_err(|err| AccountError::MailboxList(err.to_string()))?;
    let folders = match listing.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    };
    let role_folders: Vec<MailboxId> = folders
        .into_iter()
        .filter(|mailbox| {
            mailbox
                .role
                .as_ref()
                .is_some_and(|role| SYNCED_ROLES.contains(role))
        })
        .map(|mailbox| mailbox.id)
        .collect();

    // Each provider self-heals: on a dropped connection it re-dials a fresh session and
    // retries, so Refresh / opening a message recovers without an app restart.
    let renewal = auth_renewal(account);
    let mut providers: Vec<Box<dyn Provider>> = vec![Box::new(ReconnectingImapProvider::adopt(
        inbox,
        inbox_id.clone(),
        make_imap_redial(
            account.clone(),
            tokens.cloned(),
            since,
            inbox_id,
            tls.clone(),
        ),
        renewal,
    ))];
    for id in role_folders {
        // Every folder after the first dials with its own freshly resolved credential, which
        // on an OAuth account is the same cached token until it nears expiry.
        let config = dial_config(account, tokens, since).await?;
        let provider = ImapProvider::connect(&config, tls.connector(), id.clone()).await?;
        let provider: Arc<dyn Provider> = Arc::new(provider);
        let redial = make_imap_redial(
            account.clone(),
            tokens.cloned(),
            since,
            id.clone(),
            tls.clone(),
        );
        providers.push(Box::new(ReconnectingImapProvider::adopt(
            provider, id, redial, renewal,
        )));
    }
    Ok(providers)
}
