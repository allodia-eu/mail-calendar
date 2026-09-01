//! One action applied to every row the user has selected, as a single batch.
//!
//! The single-row handlers in [`super`] each re-sync the account they touched, which is right for
//! one swipe and wrong for fifty rows at once: fifty archives would be fifty account-wide syncs
//! against the user's own server. So a selection takes this path instead; one optimistic hide for
//! the whole batch, the writes, then **one** re-sync per account it reached.
//!
//! Conversations are expanded here rather than in a client. A threaded row's members come from
//! the store's thread index, which holds messages the windowed list never listed, and a move must
//! leave a copy filed in Sent where it is; neither is knowable from the snapshot a client holds.

use std::collections::HashSet;

use engine_api::{AccountId, MailEdit, MailListRow, Mailbox, MailboxRole, Provider, ProviderKey};
use engine_core::ids::MailboxId;

use super::folders::{folder_name_matches_role, resolve_move_target};
use crate::{
    App, BulkAction,
    reference::{RowRef, ThreadRef},
};

impl<P: Provider> App<P> {
    /// Applies `action` to every row in `rows`, account by account.
    ///
    /// Rows may span accounts, since the unified list selects across them, and a write only ever
    /// routes within the account its row named. An account whose part of the batch cannot be
    /// applied (no Archive folder, say) is logged and skipped; the other accounts still act.
    pub(crate) async fn act_on_selection(&self, rows: Vec<RowRef>, action: BulkAction) {
        for (account, rows) in by_account(rows) {
            match action {
                BulkAction::MarkRead => {
                    self.bulk_keyword(&account, &rows, |key| MailEdit::mark_seen(key, true))
                        .await;
                }
                BulkAction::MarkUnread => {
                    self.bulk_keyword(&account, &rows, |key| MailEdit::mark_seen(key, false))
                        .await;
                }
                BulkAction::Flag => {
                    self.bulk_keyword(&account, &rows, |key| MailEdit::set_flagged(key, true))
                        .await;
                }
                BulkAction::Unflag => {
                    self.bulk_keyword(&account, &rows, |key| MailEdit::set_flagged(key, false))
                        .await;
                }
                BulkAction::Archive => {
                    self.bulk_move(&account, &rows, Some(MailboxRole::Archive))
                        .await;
                }
                BulkAction::Delete => {
                    self.bulk_move(&account, &rows, Some(MailboxRole::Trash))
                        .await;
                }
                BulkAction::PermanentlyDelete => self.bulk_move(&account, &rows, None).await,
            }
        }
    }

    /// Every message on `thread`, straight from the store's thread index, so a conversation shown
    /// in a windowed list still resolves the messages the window cut off.
    ///
    /// A message the server never threaded projects under its own provider key as the thread id
    /// (the view-model's grouping convention), so the store holds no thread for it and the index
    /// read finds nothing; that lone message is resolved by key instead.
    pub(super) async fn thread_members(&self, thread: &ThreadRef) -> Vec<MailListRow> {
        let accounts = std::slice::from_ref(&thread.account);
        let members = self
            .engine
            .mail_on_threads(accounts, [thread.thread_id.as_str()])
            .await
            .unwrap_or_default();
        if !members.is_empty() {
            return members;
        }
        let Ok(key) = ProviderKey::new(thread.thread_id.clone()) else {
            return Vec::new();
        };
        self.engine
            .mail_by_keys(&thread.account, std::slice::from_ref(&key))
            .await
            .unwrap_or_default()
    }

    /// The in-place edits (read/unread, flag/unflag): every message on every selected row, a
    /// conversation's Sent copies included, then one re-sync.
    ///
    /// No optimistic hide: nothing leaves the list, and the rows repaint from the rebuilt
    /// snapshot the re-sync publishes.
    async fn bulk_keyword(
        &self,
        account: &AccountId,
        rows: &[RowRef],
        build: impl Fn(ProviderKey) -> MailEdit,
    ) {
        let mut keys = Vec::new();
        for row in rows {
            match row {
                RowRef::Message(message) => keys.push(message.key.clone()),
                RowRef::Thread(thread) => keys.extend(
                    self.thread_members(thread)
                        .await
                        .into_iter()
                        .map(|member| member.mail.key),
                ),
            }
        }
        let keys = deduplicated(keys);
        if keys.is_empty() {
            return;
        }
        for key in keys {
            self.edit_only(account, &build(key)).await;
        }
        self.refresh_after_write(account).await;
    }

    /// The removals (archive, trash, permanent delete): one hide for the whole batch so the rows
    /// leave the list before any network round-trip, then the writes, then one re-sync.
    ///
    /// `role` names the destination folder; `None` is the permanent delete, which has none. A row
    /// the provider refuses has its hide undone individually, so one rejection does not put the
    /// rest of the batch back on screen.
    async fn bulk_move(&self, account: &AccountId, rows: &[RowRef], role: Option<MailboxRole>) {
        let mailboxes = self.engine.mailboxes(account).await.unwrap_or_default();
        let destination = match &role {
            Some(role) => {
                let Some(mailbox) = resolve_move_target(&mailboxes, role) else {
                    log::warn!(
                        "selection: account {} has no {role:?} folder (no SPECIAL-USE role and no \
                         conventional name); skipping",
                        account.as_str(),
                    );
                    return;
                };
                Some(mailbox.id.clone())
            }
            None => None,
        };
        let settled = settled_keys(&mailboxes, destination.as_ref());
        let mut keys = Vec::new();
        for row in rows {
            match row {
                // A message the user selected by itself moves wherever they sent it, even out of
                // Sent: they named that one message. The Sent rule below is about the members a
                // *conversation* row stands for, which they did not name one by one.
                RowRef::Message(message) => keys.push(message.key.clone()),
                RowRef::Thread(thread) => keys.extend(
                    self.thread_members(thread)
                        .await
                        .into_iter()
                        .filter(|member| {
                            !member
                                .mailboxes
                                .iter()
                                .any(|id| settled.contains(id.as_str()))
                        })
                        .map(|member| member.mail.key),
                ),
            }
        }
        let keys = deduplicated(keys);
        if keys.is_empty() {
            return;
        }
        self.hide_rows(account, &keys);
        self.rebuild_snapshot().await;
        for key in &keys {
            let edit = match &destination {
                Some(destination) => MailEdit::move_to(key.clone(), destination.clone()),
                None => MailEdit::delete(key.clone()),
            };
            if !self.edit_only(account, &edit).await {
                log::warn!(
                    "selection: write rejected for key {} on account {}; restoring the row",
                    key.as_str(),
                    account.as_str(),
                );
                self.restore_row(account, key);
            }
        }
        self.refresh_after_write(account).await;
    }

    /// Hides `keys` from the list optimistically, all at once, so one republish takes the whole
    /// batch off screen rather than a row at a time.
    fn hide_rows(&self, account: &AccountId, keys: &[ProviderKey]) {
        let mut removals = self
            .pending_removals
            .lock()
            .expect("pending-removals mutex poisoned");
        for key in keys {
            removals.insert((account.as_str().to_owned(), key.as_str().to_owned()));
        }
    }

    /// Undoes one row's hide, so a refused write puts that row back on the next rebuild.
    fn restore_row(&self, account: &AccountId, key: &ProviderKey) {
        self.pending_removals
            .lock()
            .expect("pending-removals mutex poisoned")
            .remove(&(account.as_str().to_owned(), key.as_str().to_owned()));
    }
}

/// Groups `rows` by owning account, keeping the order the accounts first appear in, so a batch
/// applies in the order the list showed it.
fn by_account(rows: Vec<RowRef>) -> Vec<(AccountId, Vec<RowRef>)> {
    let mut grouped: Vec<(AccountId, Vec<RowRef>)> = Vec::new();
    for row in rows {
        match grouped
            .iter_mut()
            .find(|(account, _)| account == row.account())
        {
            Some((_, rows)) => rows.push(row),
            None => grouped.push((row.account().clone(), vec![row])),
        }
    }
    grouped
}

/// The mailbox keys a conversation's members are left in rather than moved out of: the account's
/// Sent folder(s), and the destination itself.
///
/// A Sent copy never leaves Sent (`docs/list-selection.md`), so reopening an archived thread
/// still shows both the received messages and the owner's own replies; a message already in the
/// destination has nowhere to go. The Sent lookup takes the RFC 6154 role, then the conventional
/// name, matching how the destination itself is resolved.
fn settled_keys(mailboxes: &[Mailbox], destination: Option<&MailboxId>) -> HashSet<String> {
    mailboxes
        .iter()
        .filter(|mailbox| {
            mailbox.role.as_ref() == Some(&MailboxRole::Sent)
                || folder_name_matches_role(&mailbox.name, &MailboxRole::Sent)
        })
        .map(|mailbox| mailbox.id.key().as_str().to_owned())
        .chain(destination.map(|id| id.key().as_str().to_owned()))
        .collect()
}

/// The keys in first-seen order, without repeats: two selected rows can name the same message
/// (a conversation and one of its own messages), and writing to it twice would move it and then
/// fail to find it.
fn deduplicated(keys: Vec<ProviderKey>) -> Vec<ProviderKey> {
    let mut seen = HashSet::new();
    keys.into_iter()
        .filter(|key| seen.insert(key.as_str().to_owned()))
        .collect()
}
