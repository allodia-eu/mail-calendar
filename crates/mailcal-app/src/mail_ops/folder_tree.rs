//! Changing an account's folder tree: making, renaming, moving and deleting a folder.
//!
//! Every change goes through the engine's outbox (`Engine::edit_mailbox`), which reads the
//! server's folder list before it sends anything and refuses a change whose folder has moved
//! since the user saw it. So a change made offline waits in the outbox, drawn in the pane as
//! pending ([`App::queued_folder_changes`]), and one the server refuses stands on the pane as a
//! [`FolderNotice`] rather than vanishing.

use engine_api::{
    AccountId, ApiError, DrainOutcome, DrainReport, FailureClass, Mailbox, MailboxChange,
    MailboxId, MailboxPlace, MailboxRole, PendingOpKind, Provider, SyncError,
};
use mailcal_viewmodel::{
    FolderAction, FolderNameCheck, FolderNotice, FolderProblem, QueuedFolderChange,
    check_folder_name, with_folder_changes,
};

use super::folders::resolve_move_target;
use crate::{
    App, FolderIntent, helpers::generated_idempotency, reference::FolderRef, scope::Scope,
};

impl<P: Provider> App<P> {
    /// The folder half of [`dispatch`](Self::dispatch).
    pub(crate) async fn dispatch_folders(&self, intent: FolderIntent) {
        match intent {
            FolderIntent::Create {
                account,
                parent,
                name,
            } => self.create_folder(&account, parent.as_deref(), name).await,
            FolderIntent::Rename { folder, name } => self.rename_folder(&folder, name).await,
            FolderIntent::Move { folder, parent } => {
                self.move_folder(&folder, parent.as_deref()).await;
            }
            FolderIntent::Delete(folder) => self.delete_folder(&folder).await,
            FolderIntent::MoveMessages { rows, folder } => {
                self.move_to_folder(rows, &folder).await;
            }
            FolderIntent::DismissNotice => {
                self.set_folder_notice(None);
                self.rebuild_snapshot().await;
            }
        }
    }

    async fn create_folder(&self, account: &AccountId, parent: Option<&str>, name: String) {
        let stored = self.engine.mailboxes(account).await.unwrap_or_default();
        let parent = match parent {
            Some(key) => match stored.iter().find(|m| m.id.as_str() == key) {
                Some(folder) => Some(folder.id.clone()),
                // A folder that is still being made, or has gone since the pane was drawn.
                None => return,
            },
            None => None,
        };
        let change = MailboxChange::Create {
            name: name.clone(),
            parent,
        };
        self.change_folders(account, change, FolderAction::Create, name)
            .await;
    }

    async fn rename_folder(&self, folder: &FolderRef, name: String) {
        let Some((_, target)) = self.stored_folder(folder).await else {
            return;
        };
        let change = MailboxChange::Update {
            target: target.id.clone(),
            seen: MailboxPlace::of(&target),
            name,
            parent: target.parent.clone(),
        };
        self.change_folders(&folder.account, change, FolderAction::Rename, target.name)
            .await;
    }

    async fn move_folder(&self, folder: &FolderRef, parent: Option<&str>) {
        let Some((stored, target)) = self.stored_folder(folder).await else {
            return;
        };
        let parent = match parent {
            Some(key) => match stored.iter().find(|m| m.id.as_str() == key) {
                Some(destination) => Some(destination.id.clone()),
                None => return,
            },
            None => None,
        };
        // A folder dropped back where it was is not a change.
        if parent == target.parent {
            return;
        }
        let change = MailboxChange::Update {
            target: target.id.clone(),
            seen: MailboxPlace::of(&target),
            name: target.name.clone(),
            parent,
        };
        self.change_folders(&folder.account, change, FolderAction::Move, target.name)
            .await;
    }

    async fn delete_folder(&self, folder: &FolderRef) {
        let Some((stored, target)) = self.stored_folder(folder).await else {
            return;
        };
        let change = if in_trash(&stored, &target) {
            MailboxChange::Delete {
                target: target.id.clone(),
            }
        } else if let Some(trash) = resolve_move_target(&stored, &MailboxRole::Trash) {
            MailboxChange::Trash {
                target: target.id.clone(),
                seen: MailboxPlace::of(&target),
                trash: trash.id.clone(),
            }
        } else {
            // No Trash to put it in, and a delete that cannot be undone is not what was asked.
            self.set_folder_notice(Some(FolderNotice {
                account: folder.account.as_str().to_owned(),
                folder: target.name,
                action: FolderAction::Delete,
                problem: FolderProblem::Refused,
            }));
            self.rebuild_snapshot().await;
            return;
        };
        self.change_folders(&folder.account, change, FolderAction::Delete, target.name)
            .await;
    }

    /// The account's stored folders and the one `folder` names, or `None` when the store does
    /// not hold it: a folder still being made has no server key to change yet.
    async fn stored_folder(&self, folder: &FolderRef) -> Option<(Vec<Mailbox>, Mailbox)> {
        let stored = self.engine.mailboxes(&folder.account).await.ok()?;
        let target = stored.iter().find(|m| m.id.as_str() == folder.key)?.clone();
        Some((stored, target))
    }

    /// Sends one change through the outbox and settles what the pane says about it.
    async fn change_folders(
        &self,
        account: &AccountId,
        change: MailboxChange,
        action: FolderAction,
        name: String,
    ) {
        // Clone the handle and drop the read guard before the round trip.
        let Some(acct) = self.account_handle(account).await else {
            return;
        };
        let Some(provider) = acct.providers.first() else {
            return;
        };
        if !provider.connection_info().capabilities.mailbox_writes() {
            log::warn!("folders: this account's provider cannot change folders; ignored");
            return;
        }
        match self
            .engine
            .edit_mailbox(provider, account, &generated_idempotency(), &change)
            .await
        {
            Ok(write) => {
                self.set_folder_notice(None);
                self.clear_mail_reauth_required(account);
                self.follow_open_folder(account, &change, write.outcome.mailbox)
                    .await;
            }
            Err(err) => match refusal(&err) {
                // Queued: the pane draws it as pending, and a drain pass sends it.
                None => log::info!("folders: a folder change is queued until the server answers"),
                Some(problem) => {
                    log::warn!("folders: a folder change was refused: {err}");
                    self.note_mail_write_error(account, &err);
                    self.set_folder_notice(Some(FolderNotice {
                        account: account.as_str().to_owned(),
                        folder: name,
                        action,
                        problem,
                    }));
                }
            },
        }
        self.rebuild_snapshot().await;
    }

    /// Keeps the list on the folder the user had open when a change moved or removed it.
    ///
    /// A rename or move on IMAP gives the folder a new key, which the change resolved to. A
    /// folder that is gone (trashed, deleted, or inside one that moved) gives way to its
    /// account's whole mailbox, as a folder removed elsewhere would.
    pub(crate) async fn follow_open_folder(
        &self,
        account: &AccountId,
        change: &MailboxChange,
        resolved: Option<MailboxId>,
    ) {
        let open = {
            let scope = self.scope.lock().expect("scope mutex poisoned");
            match &*scope {
                Scope::Folder(folder) if &folder.account == account => folder.key.clone(),
                _ => return,
            }
        };
        if let (MailboxChange::Update { target, .. }, Some(resolved)) = (change, resolved)
            && target.as_str() == open
        {
            self.open_folder_instead(account, Some(resolved.as_str().to_owned()));
            return;
        }
        let stored = self.engine.mailboxes(account).await.unwrap_or_default();
        if !stored.iter().any(|m| m.id.as_str() == open) {
            self.open_folder_instead(account, None);
        }
    }

    fn open_folder_instead(&self, account: &AccountId, key: Option<String>) {
        *self.scope.lock().expect("scope mutex poisoned") = match key {
            Some(key) => Scope::Folder(FolderRef {
                account: account.clone(),
                key,
            }),
            None => Scope::for_account(Some(account.clone())),
        };
        self.reset_window();
    }

    /// The folder changes still in `account`'s outbox, in the order they were made: what the
    /// pane draws over the stored tree until the server has them.
    pub(crate) async fn queued_folder_changes(
        &self,
        account: &AccountId,
    ) -> Vec<QueuedFolderChange> {
        self.engine
            .outbox(account)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|row| row.kind == Some(PendingOpKind::MailboxEdit))
            .filter_map(|row| {
                let change = serde_json::from_value(row.payload).ok()?;
                Some(QueuedFolderChange {
                    op: row.id.get(),
                    change,
                })
            })
            .collect()
    }

    /// Settles the folder changes a drain pass sent: the folder list is re-read (the drainer
    /// does not touch the store), a refused change stands on the pane, and a folder the user had
    /// open follows its change.
    pub(crate) async fn settle_drained_folder_changes(
        &self,
        provider: &P,
        account: &AccountId,
        report: &DrainReport,
        queued: &[QueuedFolderChange],
    ) {
        let drained: Vec<_> = report
            .attempted
            .iter()
            .filter(|op| op.kind == PendingOpKind::MailboxEdit)
            .collect();
        if drained.is_empty() {
            return;
        }
        if let Err(err) = self.engine.reconcile_folders(provider, account).await {
            log::warn!("folders: re-reading the folder list after a drain pass failed: {err}");
        }
        let stored = self.engine.mailboxes(account).await.unwrap_or_default();
        for op in drained {
            let Some(change) = queued.iter().find(|q| q.op == op.id.get()) else {
                continue;
            };
            match &op.outcome {
                DrainOutcome::Succeeded => {
                    let resolved = op.provider_key.clone().map(MailboxId::new);
                    self.follow_open_folder(account, &change.change, resolved)
                        .await;
                }
                DrainOutcome::Failed { class } => {
                    let problem = if *class == FailureClass::Conflict {
                        FolderProblem::ChangedElsewhere
                    } else {
                        FolderProblem::Refused
                    };
                    self.set_folder_notice(Some(notice_for(
                        account,
                        &change.change,
                        &stored,
                        problem,
                    )));
                }
                // Parked for a later pass, or never readable: nothing to say yet.
                _ => {}
            }
        }
    }

    /// Whether `name` can be given to a folder inside `parent` of `account`, answered while
    /// the user types. `renaming` is the folder being renamed, which may keep its own name.
    ///
    /// Checked against what the pane draws, queued changes included, so a name the user has
    /// just given another folder offline is taken too.
    pub async fn check_folder_name(
        &self,
        account: &AccountId,
        parent: Option<&str>,
        name: &str,
        renaming: Option<&str>,
    ) -> FolderNameCheck {
        let stored = self.engine.mailboxes(account).await.unwrap_or_default();
        let queued = self.queued_folder_changes(account).await;
        let (folders, _) = with_folder_changes(&stored, &queued);
        check_folder_name(&folders, parent, name, renaming)
    }

    pub(crate) fn set_folder_notice(&self, notice: Option<FolderNotice>) {
        self.folder_pane
            .lock()
            .expect("folder-pane mutex poisoned")
            .set_notice(notice);
    }
}

/// Why a change was refused, or `None` when it is still queued and will be tried again.
pub(crate) fn refusal(err: &ApiError) -> Option<FolderProblem> {
    match err {
        ApiError::Sync(SyncError::Provider(provider)) if provider.is_retryable() => None,
        // Enqueued and not yet claimable: another change to this tree is in flight.
        ApiError::Sync(SyncError::Outbox(_)) | ApiError::Busy => None,
        err if err.is_conflict() => Some(FolderProblem::ChangedElsewhere),
        _ => Some(FolderProblem::Refused),
    }
}

/// What the notice names a queued change by: the name the user saw, or asked for.
pub(crate) fn notice_for(
    account: &AccountId,
    change: &MailboxChange,
    stored: &[Mailbox],
    problem: FolderProblem,
) -> FolderNotice {
    let (folder, action) = match change {
        MailboxChange::Create { name, .. } => (name.clone(), FolderAction::Create),
        MailboxChange::Update { seen, name, .. } if seen.name != *name => {
            (seen.name.clone(), FolderAction::Rename)
        }
        MailboxChange::Update { seen, .. } => (seen.name.clone(), FolderAction::Move),
        MailboxChange::Trash { seen, .. } => (seen.name.clone(), FolderAction::Delete),
        MailboxChange::Delete { target } => (
            stored
                .iter()
                .find(|m| &m.id == target)
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            FolderAction::Delete,
        ),
    };
    FolderNotice {
        account: account.as_str().to_owned(),
        folder,
        action,
        problem,
    }
}

/// Whether `folder` sits inside the account's Trash, where deleting it is permanent.
fn in_trash(stored: &[Mailbox], folder: &Mailbox) -> bool {
    let mut at = folder.parent.as_ref();
    // Bounded by the list's length, so a server that reports a cycle cannot hang the walk.
    for _ in 0..stored.len() {
        let Some(parent) = at.and_then(|id| stored.iter().find(|m| &m.id == id)) else {
            return false;
        };
        if parent.role == Some(MailboxRole::Trash) {
            return true;
        }
        at = parent.parent.as_ref();
    }
    false
}
