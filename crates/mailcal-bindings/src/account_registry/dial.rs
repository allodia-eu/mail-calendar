//! **The** way an account's providers are opened; one implementation, for every path.
//!
//! # Why there is only one
//!
//! There used to be four, covering the same four families over twelve call sites: the headless
//! boot had its own, the foreground/reconnect path had its own, `add_account` had its own, and the
//! OAuth completion paths had theirs. Each independently had to register the account before it
//! dialed, keep exactly one token source per account, and never write back a config parsed before
//! the dial. Three managed it; two did not, nothing could tell you which, and getting it wrong
//! costs a real account its grant, because a provider that sees a replayed refresh token treats it
//! as theft and revokes everything.
//!
//! Duplication was not the cost. The cost was that *correct* was a property each copy had to have
//! separately, so the code could not tell you whether it held. Now there is one, its only
//! constructor is [`AccountRegistry::dial`](super::AccountRegistry::dial), and an account that is
//! not registered has no dial to run.
//!
//! # What a dial is, and is not
//!
//! It is a **snapshot** (configs cloned, token sources `Arc`-shared) taken under the registry
//! lock and then used without it, so no lock is ever held across a network round trip. It opens
//! mail, and calendar/contacts where the account has them; a calendar failure is non-fatal (mail
//! comes up with an empty agenda) and is *returned* rather than logged, so both the boot path that
//! surfaces it as a diagnostic and the reconnect path that only logs it read the same value.
//!
//! It does not touch the registry, the app, or the credential store. Deciding what a failure
//! *means* (an outage badge, a re-auth prompt, a placeholder kept in the switcher) is the
//! caller's, and differs between a launch and a mid-session retry.

use std::sync::Arc;

use engine_api::{AccountId, EmailAddress, Provider, TimeZoneId};
use futures::{StreamExt, stream};
use mailcal_account::{AccountConfig, AccountError, GraphTokenSource, JmapAccountConfig};
use mailcal_app::Account;

use crate::{BoxedAccount, ConnectedAccount, boot};

/// The most accounts dialed at once, shared by every path that dials more than one.
///
/// The two boot modes previously had *opposite* mistakes here. The foreground dialed accounts
/// **sequentially** (a `for` loop with an `await` inside) so on a five-account device the fifth
/// mailbox came alive after four full logins had finished, one after another. The headless worker
/// used an **unbounded** `join_all`, which is where the 10–11-simultaneous-connection bursts after
/// a network transition came from, measured on a production device.
///
/// Three is a compromise with a reason on each side. Above it, a device with several accounts on
/// one provider starts looking like a client that is misbehaving; Dovecot's default
/// `mail_max_userip_connections` is 10 *per user per IP*, and each account is several folders wide
/// until the engine's connection pool lands. Below it, the last account on a busy device waits for
/// no reason. It is one constant because the number is a statement about how much of the network we
/// are willing to use at once, and that cannot differ by which code path happened to start the
/// dial.
pub(crate) const MAX_CONCURRENT_DIALS: usize = 3;

/// Why a dial failed, in the **one** distinction its caller must act on differently: a stored
/// sign-in the server *refused* is not an outage.
///
/// A dial that fails because the credential was rejected ([`AccountError::SigninRejected`]: a dead
/// OAuth grant, an IMAP password answered `[AUTHENTICATIONFAILED]`, a JMAP `401`) can never succeed
/// on retry, so badging it "can't reach this account's server" is both wrong and a dead end: the
/// server *was* reached and answered. It raises the reconnect prompt instead
/// (`docs/provider-oauth.md` rule 12).
///
/// The verdict is carried as a **field decided at the `AccountError`**, not re-derived from the
/// rendered message downstream, that same rule forbids classifying on error text, and the string
/// is all that survives the boundary otherwise. Which credential a family stores is **not** part of
/// the distinction: an OAuth-shaped verdict here badges a refused password as an outage, telling
/// the user to wait for a server that has already answered.
#[derive(Debug)]
pub(crate) struct ConnectFailure {
    /// The rendered cause, for the outage badge's technical detail and the log.
    detail: String,
    /// Whether the server refused the stored sign-in, rather than being unreachable.
    signin_expired: bool,
}

impl ConnectFailure {
    /// Whether the server refused the stored sign-in: the caller raises the reconnect prompt
    /// instead of an outage badge.
    pub(crate) const fn signin_expired(&self) -> bool {
        self.signin_expired
    }
}

impl From<AccountError> for ConnectFailure {
    fn from(err: AccountError) -> Self {
        Self {
            signin_expired: matches!(err, AccountError::SigninRejected(_)),
            detail: err.to_string(),
        }
    }
}

impl std::fmt::Display for ConnectFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

/// A dial that opened the account's mail, plus the two things about it a caller may need to act on.
pub(crate) struct DialOutcome {
    /// The account with its live providers.
    pub(crate) account: BoxedAccount,
    /// The account-labelled error from an optional calendar connect that failed; mail is up and
    /// the agenda is empty. Returned rather than logged so a launch can surface it as a
    /// diagnostic while a mid-session retry just notes it; an empty calendar is otherwise
    /// indistinguishable from a server that has none.
    pub(crate) calendar_error: Option<String>,
    /// Whether a Microsoft account's calendar was withheld by a scope-denied `403`, i.e. its grant
    /// predates `Calendars.ReadWrite` and the user must re-consent. `false` for every other family
    /// and every other outcome.
    pub(crate) calendar_reauth_required: bool,
}

/// How to rebuild one registered account's providers: the credential-bearing half of its
/// [`ConnectedAccount`], snapshotted so the dial can run without the registry lock.
///
/// Obtainable **only** from [`AccountRegistry::dial`](super::AccountRegistry::dial); see this
/// module's header for why that matters.
pub(crate) enum AccountDial {
    /// An IMAP/SMTP/CalDAV account: dial from its config.
    Imap {
        /// The persisted config.
        config: AccountConfig,
        /// The shared token source, for an OAuth account only.
        tokens: Option<Arc<GraphTokenSource>>,
    },
    /// A Microsoft account: bind its Graph folder providers through the shared token source.
    Microsoft {
        /// The shared, self-refreshing token source every folder provider uses.
        tokens: Arc<GraphTokenSource>,
        /// The account's send/display identity (its Graph config carries no `imap.username`).
        identity: EmailAddress,
    },
    /// A Google account: bind its account-global Gmail provider (+ calendar) through the shared
    /// token source.
    Google {
        /// The shared, self-refreshing token source the Gmail + calendar providers use.
        tokens: Arc<GraphTokenSource>,
        /// The account's send/display identity (its Google config carries no `imap.username`).
        identity: EmailAddress,
    },
    /// A JMAP account: dial its account-wide mail provider (+ calendar when advertised) from its
    /// config, minting a fresh access token first when the account is OAuth.
    Jmap {
        /// The persisted config.
        config: JmapAccountConfig,
        /// The shared token source, for an OAuth account only.
        tokens: Option<Arc<GraphTokenSource>>,
    },
}

impl AccountDial {
    /// Snapshots the dial from a registry entry. `pub(super)` on purpose: the registry is the only
    /// thing that may build one.
    pub(super) fn from_entry(entry: &ConnectedAccount) -> Self {
        match entry {
            ConnectedAccount::Imap { config, tokens } => Self::Imap {
                config: config.clone(),
                tokens: tokens.clone(),
            },
            ConnectedAccount::Microsoft { config, tokens } => Self::Microsoft {
                tokens: Arc::clone(tokens),
                identity: config.identity(),
            },
            ConnectedAccount::Google { config, tokens } => Self::Google {
                tokens: Arc::clone(tokens),
                identity: config.identity(),
            },
            ConnectedAccount::Jmap { config, tokens } => Self::Jmap {
                config: config.clone(),
                tokens: tokens.clone(),
            },
        }
    }

    /// The provider family this dial will build, safe for a diagnostic log because it names only
    /// the account type, not an endpoint or a user identity.
    pub(crate) const fn account_type(&self) -> &'static str {
        match self {
            Self::Imap { .. } => "imap",
            Self::Microsoft { .. } => "graph",
            Self::Google { .. } => "google",
            Self::Jmap { .. } => "jmap",
        }
    }

    /// The account's address label for an outage detail (the connect error shown behind the
    /// "details" link), so a failed dial **names** the account. This is UI detail, not a log line,
    /// so carrying the address here is fine: the logs use `account[{index}]`.
    pub(crate) fn label(&self) -> String {
        match self {
            Self::Imap { config, .. } => config.imap.username.clone(),
            Self::Microsoft { identity, .. } | Self::Google { identity, .. } => {
                identity.email.clone()
            }
            Self::Jmap { config, .. } => config.identity().email,
        }
    }

    /// Opens a provider for **one** folder of this account, for the app's on-demand navigation into
    /// a folder the eager bind skipped. `None` on any failure, so the app leaves the folder empty
    /// rather than failing navigation.
    ///
    /// Here rather than in the connector that calls it because it is the same question the rest of
    /// this type answers (*what does this family need in order to open something*) and the
    /// connector had grown a private enum with the same four variants, cloned out of the registry
    /// the same way. That was the fifth copy.
    pub(crate) async fn connect_folder(self, mailbox_key: &str) -> Option<Box<dyn Provider>> {
        match self {
            Self::Imap { config, tokens } => {
                mailcal_account::connect_imap_mailbox(&config, tokens.as_ref(), mailbox_key, None)
                    .await
                    .ok()
            }
            // Graph binds the folder unwindowed; the app passes the depth per sync.
            Self::Microsoft { tokens, .. } => {
                mailcal_account::connect_graph_folder(tokens, mailbox_key, None).ok()
            }
            // Gmail's provider is account-wide (one scope covers every label), so (like JMAP) the
            // reconnected provider serves any `mailbox_key`; there is no per-folder binding.
            Self::Google { tokens, .. } => {
                mailcal_account::connect_google_folder(tokens, None).ok()
            }
            // JMAP's provider is account-wide for the same reason.
            Self::Jmap { config, tokens } => {
                mailcal_account::connect_jmap_folder(&config, tokens.as_ref())
                    .await
                    .ok()
            }
        }
    }

    /// Opens the account's providers: mail, plus calendar and contacts where it has them.
    ///
    /// `display_zone` is the `Prefer: outlook.timezone` a Microsoft account binds its Graph
    /// calendar with. A calendar failure is non-fatal and reported in the [`DialOutcome`]; a
    /// **mail** failure is the whole account's failure.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectFailure`] when the mail connect fails, carrying whether the stored sign-in
    /// is dead rather than the server unreachable.
    pub(crate) async fn run(
        self,
        id: &AccountId,
        display_zone: TimeZoneId,
    ) -> Result<DialOutcome, ConnectFailure> {
        match self {
            Self::Imap { config, tokens } => {
                let providers =
                    mailcal_account::connect_mail_providers(&config, tokens.as_ref(), id, None)
                        .await
                        .map_err(ConnectFailure::from)?;
                // Calendar and contacts are optional side quests off the mail path, and both talk
                // to the same CalDAV host: so run them CONCURRENTLY rather than
                // making the mailbox wait for one and then the other.
                let (calendar, contact_providers) = tokio::join!(
                    async {
                        let mut calendar_error = None;
                        let providers: Vec<Box<dyn Provider>> = if config.caldav.is_some() {
                            match mailcal_account::connect_caldav(&config, tokens.as_ref()).await {
                                Ok(provider) => vec![provider],
                                Err(err) => {
                                    calendar_error =
                                        Some(format!("{}: {err}", config.imap.username));
                                    Vec::new()
                                }
                            }
                        } else {
                            Vec::new()
                        };
                        (providers, calendar_error)
                    },
                    boot::connect_caldav_contacts(&config, tokens.as_ref()),
                );
                let (calendar_providers, calendar_error) = calendar;
                Ok(DialOutcome {
                    account: Account {
                        id: id.clone(),
                        providers,
                        calendar_providers,
                        contact_providers,
                        identity: EmailAddress::new(config.imap.username.clone()),
                    },
                    calendar_error,
                    calendar_reauth_required: false,
                })
            }
            Self::Microsoft { tokens, identity } => {
                let providers =
                    mailcal_account::connect_graph_mail_providers(id, Arc::clone(&tokens), None)
                        .await
                        .map_err(ConnectFailure::from)?;
                // The same Graph token also syncs the calendar; a failure is non-fatal (mail up,
                // empty agenda). A scope-denied `403` sets `calendar_reauth_required`.
                let (calendar_providers, calendar_reauth_required) =
                    boot::connect_graph_calendars(id, tokens, display_zone).await;
                Ok(DialOutcome {
                    account: Account {
                        id: id.clone(),
                        providers,
                        calendar_providers,
                        // Graph contacts need an OAuth scope this build does not request
                        // (`docs/contacts.md`, Known gaps).
                        contact_providers: Vec::new(),
                        identity,
                    },
                    calendar_error: None,
                    calendar_reauth_required,
                })
            }
            Self::Google { tokens, identity } => {
                let providers =
                    mailcal_account::connect_google_mail_providers(Arc::clone(&tokens), None)
                        .await
                        .map_err(ConnectFailure::from)?;
                // Google requests both scopes at sign-in, so there is no "connected before calendar
                // support" case and never a calendar re-consent to report.
                let calendar_providers = boot::connect_google_calendars(id, tokens).await;
                Ok(DialOutcome {
                    account: Account {
                        id: id.clone(),
                        providers,
                        calendar_providers,
                        // Google People needs a restricted scope this build does not request.
                        contact_providers: Vec::new(),
                        identity,
                    },
                    calendar_error: None,
                    calendar_reauth_required: false,
                })
            }
            Self::Jmap { config, tokens } => {
                let identity = config.identity();
                let providers =
                    mailcal_account::connect_jmap_mail_providers(&config, tokens.as_ref())
                        .await
                        .map_err(ConnectFailure::from)?;
                // JMAP serves calendars from the same account when the session advertises them.
                let calendar_providers =
                    boot::connect_jmap_calendars(&config, tokens.as_ref(), &providers).await;
                let contact_providers =
                    boot::connect_jmap_contacts(&config, tokens.as_ref(), &providers).await;
                Ok(DialOutcome {
                    account: Account {
                        id: id.clone(),
                        providers,
                        calendar_providers,
                        contact_providers,
                        identity,
                    },
                    calendar_error: None,
                    calendar_reauth_required: false,
                })
            }
        }
    }
}

/// Runs `dials` at most [`MAX_CONCURRENT_DIALS`] at a time, applying `finish` to each as it lands.
///
/// One helper rather than a bound at each call site, because the bound is the point: the number is
/// a statement about how much network to use at once, and a second call site that forgot it is how
/// this came to be unbounded in one boot mode and serial in the other. Results come back in
/// completion order, so `finish` must not assume the input order; each item carries its own index
/// for logging.
pub(crate) async fn dial_all<T, F, Fut>(dials: Vec<T>, finish: F) -> Vec<Fut::Output>
where
    F: Fn(usize, T) -> Fut,
    Fut: Future,
{
    stream::iter(
        dials
            .into_iter()
            .enumerate()
            .map(|(index, item)| finish(index, item)),
    )
    .buffer_unordered(MAX_CONCURRENT_DIALS)
    .collect()
    .await
}
