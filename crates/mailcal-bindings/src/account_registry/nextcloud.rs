//! The accounts a Nextcloud sign-in made, as the registry holds them.

use mailcal_nextcloud::{AppCredentials, Root, Secret};

use super::AccountRegistry;
use crate::ConnectedAccount;

impl AccountRegistry {
    /// The account a Nextcloud sign-in at `server` as the user `user` already made, whichever
    /// login name it was made under. `None` when the server named no user, or none was made.
    pub(crate) fn nextcloud_account(&self, server: &str, user: Option<&str>) -> Option<String> {
        let user = user?;
        self.entries
            .lock()
            .expect("account registry mutex poisoned")
            .iter()
            .find_map(|(id, entry)| match entry {
                ConnectedAccount::Imap { config, .. } => config
                    .nextcloud
                    .as_ref()
                    .filter(|login| login.server == server && login.user.as_deref() == Some(user))
                    .map(|_| id.clone()),
                _ => None,
            })
    }

    /// The app password a Nextcloud sign-in stored for `id`, with the server that issued it:
    /// what withdrawing it, or asking about a wipe, presents. `None` for every other account.
    pub(crate) fn nextcloud_credentials(&self, id: &str) -> Option<(Root, AppCredentials)> {
        match self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get(id)?
        {
            ConnectedAccount::Imap { config, .. } => issued(config),
            _ => None,
        }
    }
}

/// The Nextcloud-issued credential `config` stores, with its server.
pub(crate) fn issued(config: &mailcal_account::AccountConfig) -> Option<(Root, AppCredentials)> {
    let (login, name, password) = config.nextcloud_app_password()?;
    Some((
        Root::of(&login.server)?,
        AppCredentials {
            login_name: name.to_owned(),
            app_password: Secret::new(password.expose().to_owned()),
        },
    ))
}
