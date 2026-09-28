//! An IMAP account's connections, and the providers and watches the app binds to them.
//!
//! The engine bounds an account's sockets only when every folder is bound through **one**
//! [`provider_imap::ImapAccount`]: it holds the account's connection budget, and a second one for
//! the same account is a second budget. [`ImapConnections`] is where that one lives, one per
//! registered account, so the eager dial, a folder opened on demand and every push watch all draw
//! from it.

use std::sync::{Arc, Mutex};

use engine_core::{
    ids::{AccountId, MailboxId},
    mail::MailboxRole,
    sync::SyncUpdate,
};
use engine_provider::{Provider, Watch};
use provider_imap::DEFAULT_IDLE_KEEPALIVE;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::{
    AccountConfig, AccountError,
    reconnect::{ReconnectingImapProvider, Redial},
    tls::account_tls,
};

/// The engine's account type over the TLS stream every live connection uses.
type LiveImapAccount = provider_imap::ImapAccount<TlsStream<TcpStream>>;

/// The folder roles the app binds a provider to at startup, besides the INBOX, so their messages
/// sync and render up front (Sent threads a reply with its original; Trash shows deleted mail;
/// Drafts, Archive and Junk are the other folders a user navigates first). Folder names are
/// server-specific, so each is resolved by its SPECIAL-USE role. Any **other** folder syncs **on
/// demand** when the user opens it, through [`connect_imap_mailbox`], so no folder is permanently
/// empty.
const SYNCED_ROLES: &[MailboxRole] = &[
    MailboxRole::Sent,
    MailboxRole::Drafts,
    MailboxRole::Trash,
    MailboxRole::Archive,
    MailboxRole::Junk,
];

/// One registered IMAP account's connections: the engine account every folder provider and push
/// watch of it is bound through, connected on first use.
///
/// Build one per registered account and keep it beside the account's config. A replaced config (a
/// new password) is a new registration and so a new `ImapConnections`, which is what keeps a
/// pool dialled with the old credential from serving the new one.
#[derive(Default)]
pub struct ImapConnections {
    /// The connected account, once there is one. A plain mutex: held only to read or swap the
    /// `Arc`, never across a connect.
    account: Mutex<Option<Arc<LiveImapAccount>>>,
    /// Held across a connect, so two callers finding no account open one between them.
    connecting: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for ImapConnections {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImapConnections")
            .field("connected", &self.current().is_some())
            .finish_non_exhaustive()
    }
}

impl ImapConnections {
    /// No account connected yet: the first dial or watch connects it.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Drops every resting connection, so the next call dials fresh. For a host that saw the
    /// device come back online: the sockets from before look open and are not. Open watches are
    /// untouched and fail on their own when their socket does.
    pub fn invalidate(&self) {
        if let Some(account) = self.current() {
            account.invalidate();
        }
    }

    fn current(&self) -> Option<Arc<LiveImapAccount>> {
        self.account
            .lock()
            .expect("imap account mutex poisoned")
            .clone()
    }

    /// Connects the account afresh and makes it the one every later call is bound through.
    ///
    /// This login is the account's **first** in the dial, so a refusal here is the one that can
    /// mean the password is wrong ([`AccountError::from_first_imap_login`]). The account it
    /// replaces keeps serving whatever is still bound to it, minus its resting connections.
    async fn connect(&self, config: &AccountConfig) -> Result<Arc<LiveImapAccount>, AccountError> {
        let _connecting = self.connecting.lock().await;
        let account = Arc::new(connect_account(config).await?);
        let replaced = self
            .account
            .lock()
            .expect("imap account mutex poisoned")
            .replace(Arc::clone(&account));
        if let Some(replaced) = replaced {
            replaced.invalidate();
        }
        Ok(account)
    }

    /// The connected account, connecting it first when nothing has yet.
    async fn current_or_connect(
        &self,
        config: &AccountConfig,
    ) -> Result<Arc<LiveImapAccount>, AccountError> {
        if let Some(account) = self.current() {
            return Ok(account);
        }
        let _connecting = self.connecting.lock().await;
        // Another caller may have connected while this one waited for the lock.
        if let Some(account) = self.current() {
            return Ok(account);
        }
        let account = Arc::new(connect_account(config).await?);
        *self.account.lock().expect("imap account mutex poisoned") = Some(Arc::clone(&account));
        Ok(account)
    }
}

/// Logs in over the account's TLS policy, reading a refusal as the first login's.
async fn connect_account(config: &AccountConfig) -> Result<LiveImapAccount, AccountError> {
    let tls = account_tls(config)?;
    LiveImapAccount::connect(&config.imap_config(), tls.connector())
        .await
        .map_err(|err| AccountError::from_first_imap_login(err).over_tls(&tls))
}

/// Parses a stored mailbox key.
fn mailbox_id(mailbox: &str) -> Result<MailboxId, AccountError> {
    MailboxId::try_from(mailbox).map_err(|err| AccountError::Mailbox(err.to_string()))
}

/// Binds `mailbox` of an account through its `imap` connections, wrapped so a call that fails on
/// a dead socket is retried once.
fn bind(imap: &Arc<LiveImapAccount>, mailbox: MailboxId) -> Box<dyn Provider> {
    let provider: Arc<dyn Provider> = Arc::new(imap.provider(mailbox.clone()));
    let redial = make_imap_redial(Arc::clone(imap), mailbox.clone());
    Box::new(ReconnectingImapProvider::adopt(provider, mailbox, redial))
}

/// The [`Redial`] a [`ReconnectingImapProvider`] runs after a call failed on a dead socket.
///
/// Binding a folder dials nothing, so this only drops the account's resting connections before
/// handing back a fresh binding. The one that failed is already gone, and the others rested
/// through whatever killed it, so the retry is given a connection dialled for it rather than
/// another from the same shelf.
fn make_imap_redial(imap: Arc<LiveImapAccount>, mailbox: MailboxId) -> Redial {
    Box::new(move || {
        imap.invalidate();
        let provider: Arc<dyn Provider> = Arc::new(imap.provider(mailbox.clone()));
        Box::pin(async move { Ok(provider) })
    })
}

/// Opens one IMAP `mailbox` of `account` through its `connections`, for the host's on-demand
/// "sync the folder you open" path. Dials nothing when the account is already connected.
///
/// # Errors
///
/// Returns [`AccountError`] if `mailbox` is not a valid id, or the account had to be connected
/// and the connection or login failed.
pub async fn connect_imap_mailbox(
    connections: &ImapConnections,
    account: &AccountConfig,
    mailbox: &str,
) -> Result<Box<dyn Provider>, AccountError> {
    let mailbox = mailbox_id(mailbox)?;
    let imap = connections.current_or_connect(account).await?;
    Ok(bind(&imap, mailbox))
}

/// Opens a standing IMAP `IDLE` watch on one `mailbox` of `account`, returning it boxed behind the
/// engine's neutral [`Watch`] contract. The watch holds one of the account's connections for as
/// long as it lives (a connection in `IDLE` cannot `FETCH`), so the host drives it from its own
/// task and runs the mailbox's sync on the account's other connections when it reports
/// [`WatchEvent`](engine_provider::WatchEvent)`::Changed`. The keep-alive is the engine's
/// RFC 2177-safe default.
///
/// # Errors
///
/// Returns [`AccountError`] if `mailbox` is not a valid id, the connection or login fails, or the
/// engine refuses the watch: the server does not advertise `IDLE`, or the account has no
/// connection left to spare for it (the host then falls back to polling).
pub async fn connect_imap_watcher(
    connections: &ImapConnections,
    account: &AccountConfig,
    mailbox: &str,
) -> Result<Box<dyn Watch>, AccountError> {
    let mailbox = mailbox_id(mailbox)?;
    let imap = connections.current_or_connect(account).await?;
    let watcher = imap
        .watch(mailbox, DEFAULT_IDLE_KEEPALIVE)
        .await
        .map_err(|err| AccountError::Watch(err.to_string()))?;
    Ok(Box::new(watcher))
}

/// Connects `account` afresh through its `connections` and binds the folders the app syncs: the
/// INBOX plus every folder carrying one of the roles in `SYNCED_ROLES`, each resolved by its role
/// from the account's folder list. Returns one boxed provider per bound mailbox (just the INBOX
/// when none of the roles exist). All of them share the account's connections, so binding the role
/// folders opens no socket of its own.
///
/// # Errors
///
/// Returns [`AccountError`] if the connection or login fails, or the folder list cannot be
/// fetched.
pub async fn connect_mail_providers(
    connections: &ImapConnections,
    account: &AccountConfig,
    account_id: &AccountId,
) -> Result<Vec<Box<dyn Provider>>, AccountError> {
    let imap = connections.connect(account).await?;
    let inbox_id = mailbox_id("INBOX")?;
    let inbox = imap.provider(inbox_id.clone());

    // Enumerate folders to find the role mailboxes (their names vary by server).
    let listing = inbox
        .sync_mailboxes(account_id, None)
        .await
        .map_err(|err| AccountError::MailboxList(err.to_string()))?;
    let folders = match listing.update {
        SyncUpdate::Snapshot { objects, .. } => objects,
        SyncUpdate::Delta { changed, .. } => changed,
    };
    let role_folders = folders.into_iter().filter_map(|mailbox| {
        mailbox
            .role
            .as_ref()
            .is_some_and(|role| SYNCED_ROLES.contains(role))
            .then_some(mailbox.id)
    });
    Ok(std::iter::once(inbox_id)
        .chain(role_folders)
        .map(|mailbox| bind(&imap, mailbox))
        .collect())
}
