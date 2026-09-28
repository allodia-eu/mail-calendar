//! The folders an account lists that none of its providers is bound to.
//!
//! An account's providers are bound when it connects, one per folder wherever the protocol syncs
//! mail per folder (IMAP, Graph). A folder created after that, on the server or by another
//! client, is in the folder list the next pass fetches and in no provider, so the pass would list
//! it and never sync it. This binds each such folder through the host's
//! [`MailboxConnector`](crate::MailboxConnector) and syncs it in the same pass. Binding costs no
//! connection on either protocol, so nothing here is kept between passes.

use std::collections::BTreeSet;

use engine_api::{
    AccountId, Engine, MailSyncReport, Mailbox, MailboxRole, Provider, StreamTuning, SyncCommit,
    SyncObserver, SyncScope,
};
use engine_core::ids::MailboxId;
use futures::future::join_all;
use mailcal_account::pass_syncs;

use crate::{Account, MailboxConnector};

/// The folder a mail scope is bound to, or `None` for a scope that covers the whole account
/// (JMAP, Gmail).
pub(crate) fn scope_folder(scope: &SyncScope) -> Option<&MailboxId> {
    match scope {
        SyncScope::ImapMailbox { mailbox, .. }
        | SyncScope::GraphFolder {
            folder: mailbox, ..
        } => Some(mailbox),
        _ => None,
    }
}

/// Whether the folder `scope` is bound to is still in the account's folder list. A scope that
/// names no folder covers the account and is always listed, and so is the `INBOX`, which IMAP
/// binds by its reserved name whatever the listed row calls it.
pub(crate) fn still_listed(listed: &[Mailbox], scope: &SyncScope) -> bool {
    scope_folder(scope).is_none_or(|folder| {
        folder.as_str().eq_ignore_ascii_case("INBOX")
            || listed.iter().any(|mailbox| &mailbox.id == folder)
    })
}

/// Syncs the folders in `listed`, the account's folder list as the pass that has just run left
/// it, that `account` has no provider bound to, and folds what that did into `report`.
///
/// A no-op for an account whose provider covers every folder (JMAP, Gmail), which has nothing
/// unbound, and without a connector (the demo, tests).
pub(crate) async fn sync_unbound_folders<P: Provider, K: SyncObserver>(
    engine: &Engine,
    account: &Account<P>,
    listed: &[Mailbox],
    connector: Option<&dyn MailboxConnector<P>>,
    tuning: StreamTuning,
    observer: &K,
    report: &mut MailSyncReport,
) {
    let Some(connector) = connector else {
        return;
    };
    let Some(bound) = bound_folders(&account.id, &account.providers) else {
        return;
    };
    let unbound: Vec<&MailboxId> = listed
        .iter()
        // The Inbox is bound by its reserved name, which the listed row need not spell the
        // same way; it is bound whatever the row says.
        .filter(|mailbox| {
            mailbox.role != Some(MailboxRole::Inbox)
                && pass_syncs(mailbox)
                && !bound.contains(&mailbox.id)
        })
        .map(|mailbox| &mailbox.id)
        .collect();
    if unbound.is_empty() {
        return;
    }

    let providers: Vec<P> = join_all(
        unbound
            .iter()
            .map(|mailbox| connector.connect_folder(&account.id, mailbox.key().as_str())),
    )
    .await
    .into_iter()
    .flatten()
    .collect();
    log::debug!(
        "sync: {} folder(s) listed since the account connected, {} bound",
        unbound.len(),
        providers.len(),
    );
    if providers.is_empty() {
        return;
    }
    let added = engine
        .refresh_folders(&providers, &account.id, tuning, &SamePass(observer))
        .await;
    report.folders.extend(added.folders);
    report.elapsed += added.elapsed;
    if let Err(err) = added.account_steps
        && report.account_steps.is_ok()
    {
        report.account_steps = Err(err);
    }
}

/// The folders `providers` are bound to, or `None` when one of them covers the whole account and
/// so every folder is bound already.
fn bound_folders<P: Provider>(account: &AccountId, providers: &[P]) -> Option<BTreeSet<MailboxId>> {
    providers
        .iter()
        .map(|provider| scope_folder(&provider.email_scope(account)).cloned())
        .collect()
}

/// The pass's own observer, minus the start and end of an account pass.
///
/// The folders synced here belong to the pass that has already reported its end, so reporting a
/// second start would put the account back in the progress hint with these folders as its whole
/// total, after the hint had cleared (`docs/sync-progress.md`).
struct SamePass<'a, K>(&'a K);

impl<K: SyncObserver> SyncObserver for SamePass<'_, K> {
    fn committed(&self, commit: &SyncCommit<'_>) {
        self.0.committed(commit);
    }

    fn folder_sync_finished(&self, account: &AccountId, scope: &SyncScope, synced: bool) {
        self.0.folder_sync_finished(account, scope, synced);
    }
}
