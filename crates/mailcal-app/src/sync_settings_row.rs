//! One account's row of the sync settings: how its mail arrives, how much of it this device keeps
//! and the name it sends under, as the account's page in Settings draws it.

use std::collections::HashSet;

use engine_api::{AccountId, Provider};
use mailcal_account::{AccountSyncSettings, MAX_PUSH_FOLDERS, SyncStrategy};
use mailcal_viewmodel::{
    AccountSyncRow, SyncFolderRow, SyncStrategyKind, folder_paths, sorted_folder_rows,
};

use super::{kind, resolve};
use crate::{
    App, accounts::Account, message_size::default_size_limit_mb,
    sender_identity::sender_name_editable,
};

impl<P: Provider> App<P> {
    /// `id`'s row, or `None` for an account that is gone or not used for mail. `going` is the
    /// strategy a running switch is moving the account to: the row is then drawn as the switch
    /// will store it, its watched folders included.
    pub async fn sync_settings_row(
        &self,
        id: &str,
        going: Option<SyncStrategyKind>,
    ) -> Option<AccountSyncRow> {
        let account = self.account_handle(&AccountId::try_from(id).ok()?).await?;
        if !account.uses_mail {
            return None;
        }
        let stored = match going {
            Some(strategy) => Some(self.strategy_entry(id, strategy).await?),
            None => None,
        };
        Some(self.sync_row(&account, stored).await)
    }

    /// `account`'s row, from `going` in place of its stored settings when given.
    pub(crate) async fn sync_row(
        &self,
        account: &Account<P>,
        going: Option<AccountSyncSettings>,
    ) -> AccountSyncRow {
        // IDLE is a server property, so any provider's capability answers for the account;
        // checking all is cheap and robust to which folder bound first.
        let idle_supported = account
            .providers
            .iter()
            .any(|p| p.connection_info().capabilities.idle());
        let mailboxes = self.engine.mailboxes(&account.id).await.unwrap_or_default();
        let (stored, sync_depth_months) = {
            let guard = self
                .sync_settings
                .lock()
                .expect("sync-settings mutex poisoned");
            (
                going.or_else(|| guard.get(account.id.as_str())),
                u16::from(guard.effective_depth(account.id.as_str())),
            )
        };
        let eff = resolve(stored.as_ref(), idle_supported, &mailboxes);
        let subscribed: HashSet<&str> = eff.push_folders.iter().map(String::as_str).collect();
        let is_push = matches!(eff.strategy, SyncStrategy::Push);
        // The pane's own rows, so this list is in the pane's order rather than the provider's,
        // and each name carries the folders it sits inside: the list is flat, so two folders
        // called `2024` would otherwise read as one word each.
        let tree = sorted_folder_rows(&mailboxes);
        // A level of the tree holds no mail and refuses to be selected, so it cannot be watched.
        // Left out after the paths are built, so a folder inside it still says so.
        let containers: HashSet<&str> = mailboxes
            .iter()
            .filter(|mailbox| !mailbox.selectable)
            .map(|mailbox| mailbox.id.key().as_str())
            .collect();
        let folders = tree
            .iter()
            .zip(folder_paths(&tree))
            .filter(|(row, _)| !containers.contains(row.key.as_str()))
            .map(|(row, path)| SyncFolderRow {
                subscribed: is_push && subscribed.contains(row.key.as_str()),
                name: path,
                role: row.role,
                key: row.key.clone(),
            })
            .collect();
        AccountSyncRow {
            account_id: account.id.as_str().to_owned(),
            email: account.identity.email.clone(),
            sender_name: self.sender_name(account.id.as_str()).unwrap_or_default(),
            sender_name_editable: sender_name_editable(account),
            idle_supported,
            strategy: kind(eff.strategy),
            poll_interval_mins: eff.poll_interval_mins,
            sync_depth_months,
            message_size_limit_mb: self
                .stored_size_override(account.id.as_str())
                .map_or_else(default_size_limit_mb, u16::from),
            at_push_limit: eff.push_folders.len() >= MAX_PUSH_FOLDERS,
            folders,
        }
    }
}
