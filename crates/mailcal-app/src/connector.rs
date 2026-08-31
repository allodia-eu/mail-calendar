//! The host-injected on-demand folder-sync port.
//!
//! The app is generic over `P: Provider` and cannot itself open IMAP connections; login
//! blocks, and the TLS trust policy + credentials live in the host. So when the user
//! opens a folder whose mail has not been synced (a server that doesn't tag Archive with
//! SPECIAL-USE, or any custom folder; `mailcal_account`'s eager bind covers only INBOX +
//! the role folders), the app asks this port to connect a provider bound to that one
//! folder, then streams it. The host implements it over
//! `mailcal_account::connect_imap_mailbox`; the app applies the active sync-depth window
//! per sync. A `None` connector (the in-memory demo, tests) simply disables on-demand sync.

use async_trait::async_trait;
use engine_api::AccountId;
use mailcal_account::SyncDepth;

/// Connects a provider bound to a single mailbox of an account, on demand.
///
/// Implemented by the host (over the blocking IMAP login it owns) and injected into the
/// [`App`](crate::App). `Send + Sync` so the app can call it across `.await` on its
/// multi-threaded runtime.
#[async_trait]
pub trait MailboxConnector<P>: Send + Sync {
    /// Connects a provider bound to `mailbox_key` of `account`. `depth` is the account's
    /// effective depth (its own override, else the app default); hosts that still need it for
    /// provider construction may inspect it, but the app also passes the window per sync. Returns
    /// `None` if it cannot (an unknown account, or a connection/login failure: the app then
    /// leaves the folder empty rather than failing the navigation).
    async fn connect_folder(
        &self,
        account: &AccountId,
        mailbox_key: &str,
        depth: SyncDepth,
    ) -> Option<P>;
}
