//! A calendar-and-contacts account whose password Nextcloud issued through its own sign-in
//! (`docs/nextcloud-sign-in.md`): which server issued it, who it signs in as, and the address
//! that names the account.

use serde::Deserialize;

use crate::{AccountConfig, Secret};

/// The `[nextcloud]` table. Present exactly when the account's DAV password is an app password a
/// Nextcloud sign-in obtained; every other account has none and is stored as it always was.
///
/// The password itself stays where every DAV password is, in `[caldav]` and `[carddav]`, with the
/// login name the server returned as their `username`: the server accepts the password with that
/// name and no other spelling, so it is never edited.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NextcloudLogin {
    /// The server the sign-in ran at (`https://cloud.example.org`), and the only one the
    /// password is ever sent to.
    pub server: String,
    /// The Nextcloud user id the password signs in as, when the server named it: what tells a
    /// later sign-in it is the same person, whichever name they typed in the browser.
    #[serde(default)]
    pub user: Option<String>,
    /// The address on the person's Nextcloud profile. It names the account wherever a person
    /// reads it, because under single sign-on the login name is often an opaque id.
    #[serde(default)]
    pub address: Option<String>,
}

impl NextcloudLogin {
    /// The table as it is stored, with only the keys that are set.
    pub(crate) fn to_table(&self) -> toml::Table {
        let mut table = toml::Table::new();
        table.insert("server".into(), self.server.clone().into());
        if let Some(user) = &self.user {
            table.insert("user".into(), user.clone().into());
        }
        if let Some(address) = &self.address {
            table.insert("address".into(), address.clone().into());
        }
        table
    }
}

impl AccountConfig {
    /// The address the account is read by: its Nextcloud profile's, when a Nextcloud sign-in
    /// named one, else its login.
    #[must_use]
    pub fn address(&self) -> &str {
        self.nextcloud
            .as_ref()
            .and_then(|login| login.address.as_deref())
            .unwrap_or_else(|| self.username())
    }

    /// Clones this account with a new app password from a Nextcloud sign-in: on every DAV
    /// endpoint, under the login name the server returned, at `dav_url` (the server's own DAV
    /// endpoint), and with `login` describing where it came from. A sign-in may return another
    /// spelling of the same person's name than the last one did, which is why the login changes
    /// with the password. The endpoint replaces whatever address discovery started from, which may
    /// be a `.well-known` its web server redirects off TLS.
    ///
    /// The account's id is the caller's to keep: it is derived from the login until it is
    /// pinned.
    #[must_use]
    pub fn with_nextcloud_password(
        &self,
        login_name: &str,
        app_password: &str,
        dav_url: &str,
        login: NextcloudLogin,
    ) -> Self {
        let mut updated = self.clone();
        updated.oauth = None;
        let password = Secret::new(app_password.to_owned());
        if let Some(caldav) = &mut updated.caldav {
            dav_url.clone_into(&mut caldav.base_url);
            login_name.clone_into(&mut caldav.username);
            caldav.password = Some(password.clone());
        }
        if let Some(carddav) = &mut updated.carddav {
            dav_url.clone_into(&mut carddav.base_url);
            login_name.clone_into(&mut carddav.username);
            carddav.password = Some(password);
        }
        updated.nextcloud = Some(login);
        updated
    }

    /// The app password a Nextcloud sign-in stored, with the login name it is presented under,
    /// for withdrawing it or asking whether the server wants this device wiped. `None` on every
    /// account a Nextcloud sign-in did not make.
    #[must_use]
    pub fn nextcloud_app_password(&self) -> Option<(&NextcloudLogin, &str, &Secret)> {
        let login = self.nextcloud.as_ref()?;
        let endpoint = self.caldav_endpoint().or_else(|| self.carddav_endpoint())?;
        Some((login, endpoint.username, endpoint.password?))
    }
}

#[cfg(test)]
#[path = "nextcloud_login_tests.rs"]
mod nextcloud_login_tests;
