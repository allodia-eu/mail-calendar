//! Mail send + mutation operations, each routed to the **owning** account: plain-text compose
//! and mark-read/flag/delete/archive, plus the shared send + account helpers. Sends flow
//! through the durable outbox and surface a `Sending` → `Sent`/`Failed` hint. A mutation
//! carries a [`MessageRef`] (the account and provider key bound together) so it always acts
//! within the right account ([`App::find_message_in`]); a bare key (unique only within an
//! account) is never enough. The rich compose/reply/forward sends live in
//! [`crate::mail_compose`] (a sibling `impl App` block), which reuses the send/account helpers
//! here. Split out of `lib.rs` to keep it under the 500-line limit.

use std::time::Duration;

use engine_api::{
    AccountId, Draft, EmailAddress, MailEdit, MailboxRole, Message, MessageIdHeader, MessageReport,
    Provider, ProviderKey, ReportVerdict,
};

use crate::{
    App, BulkAction,
    helpers::{generated_idempotency, generated_message_id},
    reference::{MessageRef, RowRef, ThreadRef},
};

mod bulk;
mod folders;
mod report;
pub(crate) mod result;
mod send;

use folders::resolve_move_target;
use result::MailActionError;

/// What an optimistic removal actually asks the provider to do.
///
/// A report is **not** a move wearing a different name, which is why this is an enum rather
/// than a `MailEdit` variant: a move tells one mailbox where a message lives, a report tells
/// the *provider* something about the message, and on Graph it leaves the account entirely.
/// Both hide the row the same way, so they share the optimistic path and differ only here.
#[derive(Debug, Clone)]
pub(super) enum MailWrite {
    /// A mailbox edit: a move to Trash/Archive/Junk, or a permanent delete.
    Edit(MailEdit),
    /// A report to the provider, which files the message itself.
    Report(MessageReport),
}

/// Whether a mail write drives its own account-wide re-sync, or leaves it to the caller.
///
/// The interactive path always re-syncs at once: one user action is one refresh, and the list
/// has to settle the moment they swipe. The agent adapter (`mail_ops::result`) cannot afford
/// that; fifty scripted archives would be fifty account-wide syncs, an agent-shaped denial of
/// service against the user's own server: so it defers and runs a single coalesced refresh
/// instead. Both modes take the same code path to the provider, only the follow-up differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteRefresh {
    /// Re-sync as soon as the edit lands.
    Immediate,
    /// Leave the re-sync to the caller.
    Deferred,
}

/// How long a terminal send status (`Sent`/`Failed`) lingers before the core auto-clears it
/// back to [`SendStatus::Idle`](crate::SendStatus::Idle). Matches the brief, "sending… → sent"
/// hint the clients used to time themselves; now the core owns it so every client just renders
/// `send_status()`.
pub(crate) const AUTO_CLEAR_DELAY: Duration = Duration::from_millis(2500);

impl<P: Provider> App<P> {
    /// Sends a plain-text message from the active account (the selected one, else the first)
    /// through the durable outbox, then re-syncs so the filed Sent copy appears.
    pub(super) async fn submit_mail(&self, to: String, subject: String, body: String) {
        let Some(account) = self.compose_account().await else {
            return;
        };
        let Some(identity) = self.account_identity(&account).await else {
            return;
        };
        let Some(draft) = plain_draft(&identity, vec![EmailAddress::new(to)], subject, body) else {
            return;
        };
        self.send_draft(&account, &draft).await;
    }

    /// Marks `message` read (`read = true`) or unread, on its owning account. Returns whether
    /// the edit was applied: the interactive path discards it, the agent adapter reports it.
    pub(super) async fn mark_read(&self, message: MessageRef, read: bool) -> bool {
        self.edit_keyworded(message, WriteRefresh::Immediate, |target| {
            MailEdit::mark_seen(target, read)
        })
        .await
    }

    /// Flags or unflags `message`, on its owning account. Returns whether the edit applied.
    pub(super) async fn set_flagged(&self, message: MessageRef, flagged: bool) -> bool {
        self.edit_keyworded(message, WriteRefresh::Immediate, |target| {
            MailEdit::set_flagged(target, flagged)
        })
        .await
    }

    /// Permanently deletes `message`, on its owning account (irreversible). Hides the row
    /// optimistically, like a Trash move. Returns whether the edit applied.
    pub(super) async fn permanently_delete(&self, message: MessageRef) -> bool {
        let write = MailWrite::Edit(MailEdit::delete(message.key.clone()));
        self.remove_optimistically(message, write, WriteRefresh::Immediate)
            .await
    }

    /// Deletes `message` by moving it to **its account's** Trash folder (recoverable),
    /// resolved by role. A no-op if a Trash folder can't be resolved.
    pub(super) async fn delete(&self, message: MessageRef) -> bool {
        self.move_to_role(message, MailboxRole::Trash, WriteRefresh::Immediate)
            .await
            .is_ok()
    }

    /// Archives `message` by moving it to **its account's** Archive folder, resolved by
    /// role. A no-op if an Archive folder can't be resolved.
    pub(super) async fn archive(&self, message: MessageRef) -> bool {
        self.move_to_role(message, MailboxRole::Archive, WriteRefresh::Immediate)
            .await
            .is_ok()
    }

    /// Marks `message` as spam: **reports** it to its account's provider as junk, which files
    /// it under Junk and tells the provider to treat it as a spam sample. A provider that
    /// cannot be told files it under Junk anyway. A no-op if no Junk folder can be resolved.
    pub(super) async fn mark_as_spam(&self, message: MessageRef) -> bool {
        self.report(message, ReportVerdict::Junk, WriteRefresh::Immediate)
            .await
            .is_ok()
    }

    /// Marks `message` as not spam: reports it as not-junk, which files it back in the Inbox
    /// and tells the provider it had this one wrong. A no-op if no Inbox can be resolved.
    pub(super) async fn mark_as_not_spam(&self, message: MessageRef) -> bool {
        self.report(message, ReportVerdict::NotJunk, WriteRefresh::Immediate)
            .await
            .is_ok()
    }

    /// Moves `message` to the mailbox with `role` on its owning account (the mechanism behind
    /// delete-to-Trash and archive). Resolves the destination by the RFC 6154 SPECIAL-USE role,
    /// **falling back to a conventional folder name** when the server doesn't advertise the role
    /// ; many IMAP servers tag Trash but not Archive, which would otherwise make archive a
    /// silent no-op. A no-op (logged) only when neither resolves.
    ///
    /// Returns [`MailActionError::NoTargetFolder`] when no destination resolves and
    /// [`MailActionError::Rejected`] when the move itself did not apply: the two failures the
    /// interactive path collapses into one silent no-op and the agent adapter must tell apart.
    pub(super) async fn move_to_role(
        &self,
        message: MessageRef,
        role: MailboxRole,
        refresh: WriteRefresh,
    ) -> Result<(), MailActionError> {
        let mailboxes = self
            .engine
            .mailboxes(&message.account)
            .await
            .unwrap_or_default();
        let Some(destination) = resolve_move_target(&mailboxes, &role) else {
            log::warn!(
                "move-to-role: account {} has no {role:?} folder (no SPECIAL-USE role and no \
                 conventional name); skipping",
                message.account.as_str(),
            );
            return Err(MailActionError::NoTargetFolder);
        };
        let write = MailWrite::Edit(MailEdit::move_to(
            message.key.clone(),
            destination.id.clone(),
        ));
        if self.remove_optimistically(message, write, refresh).await {
            Ok(())
        } else {
            Err(MailActionError::Rejected)
        }
    }

    /// Archives the whole conversation `thread`: moves **every** message on it to the Archive
    /// folder **except** those filed in the Sent folder: a Sent copy is never moved out of
    /// Sent, so reopening the thread from Archive still shows both the received messages and the
    /// owner's Sent replies (the view-model gathers them across folders). Messages already in
    /// Archive are left alone. One optimistic batch (the whole received side leaves the list at
    /// once) + one refresh. A no-op when the account has no Archive folder or nothing qualifies.
    ///
    /// One conversation is a selection of one, so this runs the batch path in `mail_ops::bulk`
    /// rather than a second copy of the same rules; the Sent protection and the single re-sync
    /// are defined once for both.
    pub(super) async fn archive_thread(&self, thread: ThreadRef) {
        self.act_on_selection(vec![RowRef::Thread(thread)], BulkAction::Archive)
            .await;
    }

    /// Removes `message` from the list **optimistically**: hides the row now (so the list
    /// updates the instant the user archives/deletes), applies `write` (a Trash/Archive move, a
    /// permanent delete, or a report) through the outbox, then re-syncs. The hide persists
    /// across that re-sync (even when the server hasn't reflected the expunge yet) and
    /// self-prunes once the store agrees the message is gone (see `cached_messages`). A
    /// rejected write drops the hide so the row comes back. Returns whether the write applied.
    pub(super) async fn remove_optimistically(
        &self,
        message: MessageRef,
        write: MailWrite,
        refresh: WriteRefresh,
    ) -> bool {
        let id = (
            message.account.as_str().to_owned(),
            message.key.as_str().to_owned(),
        );
        self.pending_removals
            .lock()
            .expect("pending-removals mutex poisoned")
            .insert(id.clone());
        // Republish immediately so the row leaves the list before the network round-trip.
        self.rebuild_snapshot().await;
        let applied = self.apply_only(&message.account, &write).await;
        if !applied {
            // The write didn't apply (no provider, or a rejected/stale write); undo the hide
            // so the row returns on the re-sync below.
            log::warn!(
                "remove: write didn't apply for key {} on account {}; restoring the row",
                id.1,
                id.0,
            );
            self.pending_removals
                .lock()
                .expect("pending-removals mutex poisoned")
                .remove(&id);
        }
        if refresh == WriteRefresh::Immediate {
            self.refresh_after_write(&message.account).await;
        }
        applied
    }

    /// Finds the synced [`Message`] named by `message` **within its account**, or `None`.
    /// Scoping to the reference's account is the routing fix: provider keys are only unique
    /// within an account, so two accounts can mint the same key; scanning every account
    /// would let an action hit the wrong one. Resolved by key straight from the store, so it
    /// finds a message even when a windowed list has scrolled it out of the in-memory cache.
    pub(crate) async fn find_message_in(&self, message: &MessageRef) -> Option<Message> {
        self.engine
            .messages_by_keys(&message.account, std::slice::from_ref(&message.key))
            .await
            .unwrap_or_default()
            .into_iter()
            .next()
    }

    /// The account a new message composes from when the host names none explicitly, in order:
    /// the **selected** account (its mailbox is showing, so it scopes the choice), else the
    /// persisted **default send account** (the unified all-inboxes fallback), else the first
    /// configured account.
    ///
    /// The stored default is validated against the configured set, so an account removed after
    /// being chosen degrades to "the first account" rather than dropping the send.
    pub(crate) async fn compose_account(&self) -> Option<AccountId> {
        if let Some(account) = self.scope.lock().expect("scope mutex poisoned").account() {
            return Some(account.clone());
        }
        let accounts = self.accounts.read().await;
        if let Some(preferred) = self.default_send_account()
            && let Some(account) = accounts.iter().find(|a| a.id.as_str() == preferred)
        {
            return Some(account.id.clone());
        }
        accounts.first().map(|a| a.id.clone())
    }

    /// The send-from identity for `account`, **failing the send** when it names no configured
    /// account. Used by the rich send paths, where the account may be an explicit `from` the
    /// user picked in the composer's From dropdown: if that account was removed while the
    /// composer was open, the send must surface as failed rather than silently go out as a
    /// different sender than the one chosen.
    pub(crate) async fn identity_or_fail(&self, account: &AccountId) -> Option<EmailAddress> {
        let identity = self.account_identity(account).await;
        if identity.is_none() {
            log::warn!(
                "send: account {} is not configured; failing the send rather than substituting \
                 another sender",
                account.as_str(),
            );
            self.fail_send().await;
        }
        identity
    }

    /// The send-from identity for `account`, if it is configured.
    pub(crate) async fn account_identity(&self, account: &AccountId) -> Option<EmailAddress> {
        self.accounts
            .read()
            .await
            .iter()
            .find(|a| &a.id == account)
            .map(|a| a.identity.clone())
    }

    /// Builds a keyword/permanent edit for `message` and applies it to its owning account.
    /// Used by mark-read/flag/permanent-delete. The engine scopes the edit to that account
    /// and selects the message's own mailbox by key, so the edit is a no-op when the key
    /// isn't in the account; it can never reach another account.
    async fn edit_keyworded(
        &self,
        message: MessageRef,
        refresh: WriteRefresh,
        build: impl FnOnce(ProviderKey) -> MailEdit,
    ) -> bool {
        self.apply_edit(&message.account, build(message.key), refresh)
            .await
    }

    /// Applies a [`MailEdit`] to `account` through the durable outbox, then re-syncs. Used by
    /// the in-place edits (mark-read/flag) that keep the message in view. Returns whether the
    /// edit applied.
    async fn apply_edit(&self, account: &AccountId, edit: MailEdit, refresh: WriteRefresh) -> bool {
        let applied = self.edit_only(account, &edit).await;
        if refresh == WriteRefresh::Immediate {
            self.refresh_after_write(account).await;
        }
        applied
    }

    /// Applies a [`MailEdit`] to `account` through the durable outbox (its first provider;
    /// `edit_mail` selects the message's own mailbox by key), **without** the re-sync. Returns
    /// whether the edit was applied; `false` when the account has no provider or the provider
    /// rejected it: so an optimistic caller can undo its hide.
    /// Applies `write` without touching the list: the shared tail of every optimistic
    /// removal. Returns whether it landed.
    pub(super) async fn apply_only(&self, account: &AccountId, write: &MailWrite) -> bool {
        match write {
            MailWrite::Edit(edit) => self.edit_only(account, edit).await,
            MailWrite::Report(report) => self.report_only(account, report).await,
        }
    }

    async fn edit_only(&self, account: &AccountId, edit: &MailEdit) -> bool {
        // Clone the account handle, then edit with the read guard released: the IMAP
        // round-trip must not hold the lock.
        if let Some(acct) = self.account_handle(account).await
            && let Some(provider) = acct.providers.first()
        {
            return match self
                .engine
                .edit_mail(provider, account, &generated_idempotency(), edit)
                .await
            {
                Ok(_) => {
                    // The write went through, so the grant carries the mail-write scope; clear
                    // any standing "reconnect to manage mail" prompt for this account.
                    self.clear_mail_reauth_required(account);
                    true
                }
                Err(err) => {
                    // Log the rejected edit so it is discoverable; e.g. a Graph
                    // `403 ErrorAccessDenied` when the OAuth grant lacks `Mail.ReadWrite`. The
                    // error is a class + protocol detail, never message content or addresses; the
                    // `false` still lets an optimistic caller undo its hide. An access-denied
                    // refusal additionally raises the account's mail re-consent prompt.
                    log::warn!("edit: mail edit failed: {err}");
                    self.note_mail_write_error(account, &err);
                    false
                }
            };
        }
        false
    }
}

/// Builds a plain-text draft from `identity` to `to`. Shared by the plain composer intent and
/// the agent adapter's direct send, so both mint their `Message-ID` the same way.
fn plain_draft(
    identity: &EmailAddress,
    to: Vec<EmailAddress>,
    subject: String,
    body: String,
) -> Option<Draft> {
    let message_id = MessageIdHeader::new(generated_message_id()).ok()?;
    Some(Draft::new(message_id, identity.clone(), to, subject, body))
}
