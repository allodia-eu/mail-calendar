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

use mailcal_nextcloud::{AppCredentials, Named, NextcloudError, PendingFlow, Poll, Root};
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

/// What starting a sign-in at an address a person typed as their Nextcloud server's came to.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum NextcloudAddressStart {
    /// The server started a sign-in, which goes on as any other does.
    Started { start: NextcloudSignInStart },
    /// Nothing at `host` showed whether it is Nextcloud: its `.well-known` addresses lead to no
    /// PHP endpoint, or its status could not be read, which a mistyped address shows and so can a
    /// Nextcloud server with calendars and contacts switched off. The client says so and offers
    /// the sign-in anyway ([`MailcalApp::begin_nextcloud_sign_in_anyway`]).
    Unsure { host: String },
    /// The Nextcloud server at `host` is in maintenance and runs no sign-in until it is over.
    Maintenance { host: String },
    /// The server at `host` is something else: its status names another product, or its
    /// `.well-known` addresses lead to a DAV server that runs no PHP; or, tried anyway, it started
    /// no sign-in.
    NotNextcloud { host: String },
    /// No Nextcloud server can be at what was typed: nothing, no host, plain HTTP to another
    /// machine, or an address carrying a name or password.
    NotAnAddress,
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
    /// **Blocking** (a few requests of at most three seconds each); call it off the UI thread.
    /// Never an error: a server that cannot be asked offers the password, which works everywhere.
    pub fn offers_nextcloud_sign_in(&self, server_url: String) -> bool {
        self.offers_for(&mailcal_account::normalize_caldav_base_url(&server_url))
    }

    /// Starts a Nextcloud sign-in at the server `server_url` names: any URL on it will do, a
    /// DAV endpoint included. The app password it leads to is named after this app and this
    /// device's platform, which is how the person recognises it in the browser and later in
    /// Nextcloud's list of devices.
    ///
    /// **Blocking** (the server is located, then asked to start one); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] for a URL no Nextcloud server can be at, and
    /// [`MailcalError::Connect`] when the server starts no sign-in.
    pub fn begin_nextcloud_sign_in(
        &self,
        server_url: String,
    ) -> Result<NextcloudSignInStart, MailcalError> {
        let root = self
            .root_for(&mailcal_account::normalize_caldav_base_url(&server_url))
            .ok_or_else(|| MailcalError::Config("not a Nextcloud server address".to_owned()))?;
        self.begin_at(&root)
    }

    /// Starts a Nextcloud sign-in at `address`, which the person typed as their Nextcloud
    /// server's: a host name or any URL on it. The server is asked first, as rule 1 of
    /// `docs/nextcloud-sign-in.md` allows, and only one that answers as Nextcloud is asked to
    /// start a sign-in; one that shows nothing either way is [`NextcloudAddressStart::Unsure`],
    /// and a browser opens for neither.
    ///
    /// **Blocking** (the server is located, asked and then asked to start); call it off the UI
    /// thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] when a Nextcloud server answered but started no sign-in,
    /// and [`MailcalError::Config`] when no client for the requests can be made.
    pub fn begin_nextcloud_sign_in_at(
        &self,
        address: String,
    ) -> Result<NextcloudAddressStart, MailcalError> {
        let Some((url, typed)) = typed_address(&address) else {
            return Ok(NextcloudAddressStart::NotAnAddress);
        };
        let client = setup_client()?;
        let named = self
            .runtime
            .block_on(|| async { mailcal_nextcloud::named_nextcloud_root(&client, &url).await });
        let host = typed.host().to_owned();
        match named {
            Named::Nextcloud(root) => self
                .begin_at(&root)
                .map(|start| NextcloudAddressStart::Started { start }),
            Named::Maintenance => {
                log::info!("nextcloud: the server is in maintenance");
                Ok(NextcloudAddressStart::Maintenance { host })
            }
            Named::Other => {
                log::info!("nextcloud: the address typed answered as something else");
                Ok(NextcloudAddressStart::NotNextcloud { host })
            }
            Named::Unsure => {
                log::info!("nextcloud: the address typed showed nothing either way");
                Ok(NextcloudAddressStart::Unsure { host })
            }
        }
    }

    /// Starts a Nextcloud sign-in at the server `address` names without asking whether it is
    /// Nextcloud: the person chose to try after [`NextcloudAddressStart::Unsure`]. An address
    /// with a path is tried there and then at its host, since a URL copied from the browser names
    /// a page rather than the installation. A server that starts none is
    /// [`NextcloudAddressStart::NotNextcloud`].
    ///
    /// **Blocking** (a request, or two for an address with a path); call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] when no client for the request can be made.
    pub fn begin_nextcloud_sign_in_anyway(
        &self,
        address: String,
    ) -> Result<NextcloudAddressStart, MailcalError> {
        let Some((url, typed)) = typed_address(&address) else {
            return Ok(NextcloudAddressStart::NotAnAddress);
        };
        let host = host_root(&url).filter(|host| *host != typed);
        for root in std::iter::once(typed.clone()).chain(host) {
            match self.begin_at(&root) {
                Ok(start) => return Ok(NextcloudAddressStart::Started { start }),
                Err(MailcalError::Connect(_)) => {}
                Err(other) => return Err(other),
            }
        }
        Ok(NextcloudAddressStart::NotNextcloud {
            host: typed.host().to_owned(),
        })
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
        let polled = self.runtime.block_on(|| async {
            let credentials =
                match mailcal_nextcloud::poll(&client, &pending, SystemTime::now()).await? {
                    Poll::Waiting => return Ok(NextcloudSignInPoll::Waiting),
                    Poll::Expired => {
                        log::info!("nextcloud: the sign-in was not finished in time");
                        return Ok(NextcloudSignInPoll::Expired);
                    }
                    Poll::Granted(credentials) => credentials,
                };
            let user = mailcal_nextcloud::current_user(&client, &pending.root, &credentials)
                .await
                .inspect_err(|err| {
                    log::info!("nextcloud: the server did not say who signed in ({err})");
                })
                .ok();
            log::info!("nextcloud: access granted");
            let grant = NextcloudGrant {
                root: pending.root.clone(),
                credentials,
                user: user.as_ref().map(|user| user.id.clone()),
                email: user.and_then(|user| user.email),
            };
            Ok::<_, NextcloudError>(NextcloudSignInPoll::Granted {
                grant: serde_json::to_string(&grant).map_err(|_| NextcloudError::Malformed)?,
            })
        });
        polled.map_err(|err| {
            log::warn!("nextcloud: the grant could not be read ({err})");
            MailcalError::Connect(format!("the server's answer could not be read: {err}"))
        })
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

    /// The Nextcloud server the DAV address `url` reaches: where its `.well-known` hop lands when
    /// that server answers as Nextcloud, which for a bare host may be another host or a directory
    /// below it, else the server `url` names itself. `None` for a URL no Nextcloud server can be
    /// at.
    pub(crate) fn root_for(&self, url: &str) -> Option<Root> {
        let located = setup_client().ok().and_then(|client| {
            self.runtime
                .block_on(|| async { mailcal_nextcloud::nextcloud_root(&client, url).await })
        });
        located.or_else(|| Root::of(url))
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

/// The URL a person's typed Nextcloud address names, HTTPS when it names no scheme, and the server
/// it is on; `None` for what no Nextcloud server can be at.
fn typed_address(address: &str) -> Option<(String, Root)> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    let url = mailcal_account::normalize_caldav_base_url(address);
    let root = Root::of(&url)?;
    Some((url, root))
}

/// The server at the root of `url`'s host, without its path.
fn host_root(url: &str) -> Option<Root> {
    let mut url = url::Url::parse(url).ok()?;
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    Root::of(url.as_str())
}

/// The HTTP client a sign-in runs on: the trust setup has, with no certificate exceptions.
pub(crate) fn setup_client() -> Result<reqwest::Client, MailcalError> {
    mailcal_nextcloud::client_for(&mailcal_account::setup_trust_policy())
        .map_err(|err| MailcalError::Config(err.to_string()))
}

/// The name a sign-in gives the app password: this app, and the platform it runs on.
pub(crate) fn client_name(platform: Option<Platform>) -> String {
    password_name(env!("MAILCAL_APP_NAME"), platform_label(platform))
}

/// `app` and `platform` as the app password's name, without an "&": Nextcloud's grant page
/// escapes the name twice, so one would read "&amp;" there.
pub(crate) fn password_name(app: &str, platform: &str) -> String {
    let app = app.replace('&', " ");
    let app = app.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{app} ({platform})")
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
