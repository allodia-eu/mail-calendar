//! Signing in to a Nextcloud server through its own sign-in, Login Flow v2
//! (`docs/nextcloud-sign-in.md`): the question whether a server offers it, and the browser half.
//!
//! The flow is two calls and a wait. [`MailcalApp::begin_nextcloud_sign_in`] returns a page for
//! the person's browser; the client then polls [`MailcalApp::poll_nextcloud_sign_in`] every
//! [`nextcloud_poll_interval_ms`], and once more when it returns to the foreground, until the
//! person has granted access there. The grant it hands back is completed by the surface that
//! asked (`nextcloud_accounts`), or withdrawn with
//! [`MailcalApp::abandon_nextcloud_sign_in`] when it becomes no account.

use std::time::SystemTime;

use mailcal_nextcloud::{AppCredentials, PendingFlow, Poll, Root};
use serde::{Deserialize, Serialize};

use crate::{MailcalApp, MailcalError, Platform};

/// The browser half of a Nextcloud sign-in: the page to open, and the handle to poll with.
#[derive(Debug, Clone, uniffi::Record)]
pub struct NextcloudSignInStart {
    /// The server's login page, on its own host. Opened in the person's browser, where they sign
    /// in the way they always do, single sign-on and two-factor included.
    pub login_url: String,
    /// Opaque; hand it to [`MailcalApp::poll_nextcloud_sign_in`] unchanged. It carries the poll
    /// token, so hold it in memory only.
    pub pending: String,
}

/// What one poll found.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum NextcloudSignInPoll {
    /// The person has not finished in the browser yet. Poll again.
    Waiting,
    /// The sign-in was not finished in time and is over; a new one has to start.
    Expired,
    /// The person granted access. `grant` is opaque and carries the app password, so hold it in
    /// memory only, and hand it to the completion of the surface that started the sign-in. The
    /// server hands it out once: keep it until that completion succeeds, so a failed connect can
    /// be retried without a second trip to the browser.
    Granted {
        /// Opaque; hand it back unchanged.
        grant: String,
    },
}

/// What a granted sign-in yields, round-tripped through the client.
#[derive(Serialize, Deserialize)]
pub(crate) struct NextcloudGrant {
    /// The server the flow ran at.
    pub(crate) root: Root,
    /// The login name and the app password.
    pub(crate) credentials: AppCredentials,
    /// Who the password signs in as, when the server said.
    pub(crate) user: Option<String>,
    /// The address on the person's Nextcloud profile, when it has one.
    pub(crate) email: Option<String>,
}

impl NextcloudGrant {
    /// Reads a grant handed back by a client.
    pub(crate) fn parse(grant: &str) -> Result<Self, MailcalError> {
        serde_json::from_str(grant)
            .map_err(|_| MailcalError::Config("that is not a Nextcloud sign-in".to_owned()))
    }

    /// What the account stores about where its password came from.
    pub(crate) fn login(&self) -> mailcal_account::NextcloudLogin {
        mailcal_account::NextcloudLogin {
            server: self.root.as_str().to_owned(),
            user: self.user.clone(),
            address: self.email.clone(),
        }
    }
}

/// How often a client asks whether the person has finished signing in, in milliseconds.
#[uniffi::export]
#[must_use]
pub fn nextcloud_poll_interval_ms() -> u64 {
    u64::try_from(mailcal_nextcloud::POLL_INTERVAL.as_millis()).unwrap_or(u64::MAX)
}

#[uniffi::export]
impl MailcalApp {
    /// Whether the calendar or address-book server at `server_url` is a Nextcloud server, which
    /// signs in through the person's browser. Decides whether a form offers "Sign in with
    /// Nextcloud" beside its password field.
    ///
    /// **Blocking** (one request, at most three seconds); call it off the UI thread. Never an
    /// error: a server that cannot be asked offers the password, which works everywhere.
    pub fn offers_nextcloud_sign_in(&self, server_url: String) -> bool {
        self.offers_for(&mailcal_account::normalize_caldav_base_url(&server_url))
    }

    /// Starts a Nextcloud sign-in at the server `server_url` names: any URL on it will do, a
    /// DAV endpoint included. The app password it leads to is named after this app and this
    /// device's platform, which is how the person recognises it in the browser and later in
    /// Nextcloud's list of devices.
    ///
    /// **Blocking** (one request); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for a URL no Nextcloud server can be at, and
    /// [`MailcalError::Connect`] when the server starts no sign-in.
    pub fn begin_nextcloud_sign_in(
        &self,
        server_url: String,
    ) -> Result<NextcloudSignInStart, MailcalError> {
        let root = Root::of(&mailcal_account::normalize_caldav_base_url(&server_url))
            .ok_or_else(|| MailcalError::Config("not a Nextcloud server address".to_owned()))?;
        self.begin_at(&root)
    }

    /// Asks once whether the person has finished the sign-in `pending` started. On a grant, it
    /// also asks the server who the password signs in as, which names the account.
    ///
    /// **Blocking** (one or two requests); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for a malformed `pending`, and [`MailcalError::Connect`]
    /// when the server granted access in an answer that cannot be read, which ends the sign-in.
    pub fn poll_nextcloud_sign_in(
        &self,
        pending: String,
    ) -> Result<NextcloudSignInPoll, MailcalError> {
        let pending: PendingFlow = serde_json::from_str(&pending)
            .map_err(|_| MailcalError::Config("that is not a Nextcloud sign-in".to_owned()))?;
        let client = setup_client()?;
        let granted = self.runtime.block_on(|| async {
            let credentials =
                match mailcal_nextcloud::poll(&client, &pending, SystemTime::now()).await? {
                    Poll::Waiting => return Ok(None),
                    Poll::Expired => return Ok(Some(None)),
                    Poll::Granted(credentials) => credentials,
                };
            let user = mailcal_nextcloud::current_user(&client, &pending.root, &credentials)
                .await
                .inspect_err(|err| {
                    log::info!("nextcloud: the server did not say who signed in ({err})");
                })
                .ok();
            Ok::<_, mailcal_nextcloud::NextcloudError>(Some(Some(NextcloudGrant {
                root: pending.root.clone(),
                credentials,
                user: user.as_ref().map(|user| user.id.clone()),
                email: user.and_then(|user| user.email),
            })))
        });
        match granted {
            Ok(None) => Ok(NextcloudSignInPoll::Waiting),
            Ok(Some(None)) => {
                log::info!("nextcloud: the sign-in was not finished in time");
                Ok(NextcloudSignInPoll::Expired)
            }
            Ok(Some(Some(grant))) => {
                log::info!("nextcloud: access granted");
                Ok(NextcloudSignInPoll::Granted {
                    grant: serde_json::to_string(&grant)
                        .map_err(|err| MailcalError::Config(err.to_string()))?,
                })
            }
            Err(err) => {
                log::warn!("nextcloud: the grant could not be read ({err})");
                Err(MailcalError::Connect(format!(
                    "the server's answer could not be read: {err}"
                )))
            }
        }
    }

    /// Withdraws the app password of a granted sign-in that became no account: the person left
    /// the step, or chose another way. Returns at once; the withdrawal is best effort.
    pub fn abandon_nextcloud_sign_in(&self, grant: String) {
        let Ok(grant) = NextcloudGrant::parse(&grant) else {
            return;
        };
        self.revoke_later(grant.root, grant.credentials, "an unused sign-in");
    }
}

impl MailcalApp {
    /// Whether the DAV server at `url` answers as Nextcloud, asked only when it shows a sign of
    /// being one (`mailcal_nextcloud::answers_as_nextcloud`); never an error.
    pub(crate) fn offers_for(&self, url: &str) -> bool {
        let Ok(client) = setup_client() else {
            return false;
        };
        self.runtime
            .block_on(|| async { mailcal_nextcloud::answers_as_nextcloud(&client, url).await })
    }

    /// Starts a sign-in at `root`, naming the app password after this app and platform.
    pub(crate) fn begin_at(&self, root: &Root) -> Result<NextcloudSignInStart, MailcalError> {
        let client = setup_client()?;
        let name = client_name(self.platform);
        let start = self
            .runtime
            .block_on(|| async { mailcal_nextcloud::begin(&client, root, &name).await })
            .map_err(|err| {
                log::info!("nextcloud: the server started no sign-in ({err})");
                MailcalError::Connect(format!("the server started no sign-in: {err}"))
            })?;
        log::info!("nextcloud: opening the login page; polling for the grant");
        Ok(NextcloudSignInStart {
            login_url: start.login_url,
            pending: serde_json::to_string(&start.pending)
                .map_err(|err| MailcalError::Config(err.to_string()))?,
        })
    }

    /// Withdraws `credentials` at `root` in the background, logging how it went under `what`.
    pub(crate) fn revoke_later(&self, root: Root, credentials: AppCredentials, what: &'static str) {
        self.runtime.spawn(async move {
            let Ok(client) = setup_client() else {
                return;
            };
            match mailcal_nextcloud::revoke(&client, &root, &credentials).await {
                Ok(()) => log::info!("nextcloud: withdrew the app password of {what}"),
                Err(err) => {
                    log::info!("nextcloud: could not withdraw the app password of {what} ({err})");
                }
            }
        });
    }
}

/// The HTTP client a sign-in runs on: the trust setup has, with no certificate exceptions.
pub(crate) fn setup_client() -> Result<reqwest::Client, MailcalError> {
    mailcal_nextcloud::client_for(&mailcal_account::setup_trust_policy())
        .map_err(|err| MailcalError::Config(err.to_string()))
}

/// The name a sign-in gives the app password: this app, and the platform it runs on.
pub(crate) fn client_name(platform: Option<Platform>) -> String {
    format!(
        "{} ({})",
        env!("MAILCAL_APP_NAME"),
        platform_label(platform)
    )
}

/// The platform as a person names it.
fn platform_label(platform: Option<Platform>) -> &'static str {
    match platform {
        Some(Platform::Macos) => "macOS",
        Some(Platform::Ios) => "iOS",
        Some(Platform::Ipados) => "iPadOS",
        Some(Platform::Windows) => "Windows",
        Some(Platform::Android) => "Android",
        Some(Platform::Linux) => "Linux",
        None => match std::env::consts::OS {
            "macos" => "macOS",
            "ios" => "iOS",
            "windows" => "Windows",
            "android" => "Android",
            _ => "Linux",
        },
    }
}

/// A grant whose secret is `secret`, for tests.
#[cfg(test)]
pub(crate) fn test_grant(root: &str, login: &str, secret: &str, user: Option<&str>) -> String {
    serde_json::to_string(&NextcloudGrant {
        root: Root::of(root).unwrap(),
        credentials: AppCredentials {
            login_name: login.to_owned(),
            app_password: mailcal_nextcloud::Secret::new(secret.to_owned()),
        },
        user: user.map(str::to_owned),
        email: user.map(|user| format!("{user}@example.org")),
    })
    .unwrap()
}

#[cfg(test)]
#[path = "nextcloud_tests.rs"]
pub(crate) mod nextcloud_tests;
