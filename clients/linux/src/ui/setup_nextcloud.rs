//! Signing in with Nextcloud from setup (`docs/nextcloud-sign-in.md` rule 2): from the found
//! card for a calendar and contacts server detection recognised as Nextcloud, the same card set
//! up from beside a JMAP server, and the manual calendar and contacts pane once the server typed
//! there answers as Nextcloud. The browser half is [`super::nextcloud_flight`]'s; what is added
//! is the calendar and contacts account the grant signs in to, used for what the card chose.
//!
//! A grant the core could not make an account of is kept, and the next press tries it again
//! without the browser, because the server hands it out once. Leaving setup withdraws it.

use mailcal_bindings::{
    AccountCapability, MailcalApp, MailcalError, NextcloudAddressStart, NextcloudSignInStart,
};

use super::{
    AppInput, AppModel,
    nextcloud_flight::{self, Flight, FlightEnd, Grant},
    setup_already_set_up::{ExistingAccount, refusal},
    setup_model::{ConnectFailure, ManualForm},
};
use crate::l10n;

/// Setup's traffic about a sign-in with Nextcloud.
pub(crate) enum SetupNextcloudInput {
    /// Sign in at `server`, for `uses`.
    Start {
        server: String,
        uses: Vec<AccountCapability>,
    },
    /// The manual Nextcloud pane: sign in at the address it holds, once the core has found
    /// Nextcloud there, or `anyway`, after it could not tell.
    StartAt {
        typed: Box<ManualForm>,
        anyway: bool,
    },
    /// Give up on the sign-in the browser holds.
    Cancel,
    /// How the sign-in is getting on.
    Flight {
        attempt: u64,
        server: String,
        uses: Vec<AccountCapability>,
        flight: Flight,
    },
    /// The core made an account of the grant, or did not.
    Added {
        result: Result<String, ConnectFailure>,
    },
    /// A manual calendar and contacts pane's server field was left: ask whether it is Nextcloud.
    Ask(Box<ManualForm>),
    /// The server answered.
    Answered { server: String, offered: bool },
}

/// By variant only: a manual form carries what was typed.
impl std::fmt::Debug for SetupNextcloudInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Start { .. } => "Start",
            Self::StartAt { .. } => "StartAt",
            Self::Cancel => "Cancel",
            Self::Flight { .. } => "Flight",
            Self::Added { .. } => "Added",
            Self::Ask(_) => "Ask",
            Self::Answered { .. } => "Answered",
        })
    }
}

fn input(input: SetupNextcloudInput) -> AppInput {
    AppInput::SetupNextcloud(input)
}

/// What an account signed in to from the Nextcloud pane is used for: the calendar and the address
/// book Nextcloud serves side by side.
const NEXTCLOUD_USES: [AccountCapability; 2] =
    [AccountCapability::Calendar, AccountCapability::Contacts];

/// Starts a sign-in at the address the Nextcloud pane holds, asking the server first unless
/// `anyway`, or ends the flight saying why none started.
fn at_address(
    app: &MailcalApp,
    address: String,
    anyway: bool,
) -> Result<NextcloudSignInStart, FlightEnd> {
    let started = if anyway {
        app.begin_nextcloud_sign_in_anyway(address)
    } else {
        app.begin_nextcloud_sign_in_at(address)
    };
    match started {
        Ok(NextcloudAddressStart::Started { start }) => Ok(start),
        Ok(NextcloudAddressStart::Unsure { host }) => Err(FlightEnd::Unsure { host }),
        Ok(NextcloudAddressStart::NotNextcloud { host }) => {
            Err(FlightEnd::Failed(l10n::setup_nextcloud_not_found(&host)))
        }
        Ok(NextcloudAddressStart::Maintenance { host }) => {
            Err(FlightEnd::Failed(l10n::setup_nextcloud_maintenance(&host)))
        }
        Ok(NextcloudAddressStart::NotAnAddress) => Err(FlightEnd::Failed(
            l10n::setup_nextcloud_address_invalid().to_owned(),
        )),
        Err(error) => Err(FlightEnd::Failed(nextcloud_flight::said(&error))),
    }
}

impl AppModel {
    pub(super) fn setup_nextcloud_input(
        &mut self,
        message: SetupNextcloudInput,
        sender: relm4::Sender<AppInput>,
    ) {
        match message {
            SetupNextcloudInput::Start { server, uses } => {
                let begin_at = server.clone();
                let begin = move |app: &MailcalApp| {
                    app.begin_nextcloud_sign_in(begin_at)
                        .map_err(|error| FlightEnd::Failed(nextcloud_flight::said(&error)))
                };
                self.start_setup_nextcloud(server, uses, false, begin, sender);
            }
            SetupNextcloudInput::StartAt { typed, anyway } => {
                if let Some(address) = self.setup.adopt_manual_nextcloud(*typed) {
                    let begin_at = address.clone();
                    let begin = move |app: &MailcalApp| at_address(app, begin_at, anyway);
                    let uses = NEXTCLOUD_USES.to_vec();
                    self.start_setup_nextcloud(address, uses, !anyway, begin, sender);
                }
            }
            SetupNextcloudInput::Cancel => {
                if self.host_tasks.nextcloud.cancel() {
                    self.setup.retry_form();
                }
            }
            SetupNextcloudInput::Flight {
                attempt,
                server,
                uses,
                flight,
            } => self.setup_nextcloud_flight(attempt, server, uses, flight, sender),
            SetupNextcloudInput::Added { result } => {
                if result.is_ok() {
                    self.setup.nextcloud.take();
                }
                self.account_added(result, &sender);
            }
            SetupNextcloudInput::Ask(typed) => self.ask_setup_nextcloud(*typed, sender),
            SetupNextcloudInput::Answered { server, offered } => {
                self.setup.nextcloud_answered(&server, offered);
            }
        }
    }

    /// Withdraws the grant setup kept for another try, when there is one: setup is being left,
    /// or a sign-in at another server starts.
    pub(super) fn abandon_setup_nextcloud_grant(&mut self) {
        if let (Some(grant), Some(app)) = (self.setup.nextcloud.take(), &self.app) {
            app.abandon_nextcloud_sign_in(grant.0);
        }
    }

    /// Signs in at `server` for `uses`, started by `begin`: in the browser, or straight to the
    /// account when a grant is kept for that server. `asking` when `begin` first asks the server
    /// whether it is Nextcloud.
    fn start_setup_nextcloud(
        &mut self,
        server: String,
        uses: Vec<AccountCapability>,
        asking: bool,
        begin: impl FnOnce(&MailcalApp) -> Result<NextcloudSignInStart, FlightEnd> + Send + 'static,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if let Some(grant) = self.setup.nextcloud.held_for(&server) {
            return self.add_setup_nextcloud(grant, uses, sender);
        }
        self.abandon_setup_nextcloud_grant();
        let (attempt, cancel) = self.host_tasks.nextcloud.start();
        self.setup.nextcloud_signing_in(asking);
        nextcloud_flight::fly(app, cancel, begin, move |flight| {
            sender.emit(input(SetupNextcloudInput::Flight {
                attempt,
                server: server.clone(),
                uses: uses.clone(),
                flight,
            }));
        });
    }

    fn setup_nextcloud_flight(
        &mut self,
        attempt: u64,
        server: String,
        uses: Vec<AccountCapability>,
        flight: Flight,
        sender: relm4::Sender<AppInput>,
    ) {
        let held = self.host_tasks.nextcloud.holds(attempt);
        match flight {
            Flight::Opened(login_url) if held => {
                self.setup.nextcloud_browser_opened();
                nextcloud_flight::open_browser(&login_url, move || {
                    sender.emit(input(SetupNextcloudInput::Flight {
                        attempt,
                        server,
                        uses,
                        flight: Flight::Ended(FlightEnd::Failed(
                            l10n::account_signin_browser_failed().to_owned(),
                        )),
                    }));
                });
            }
            Flight::Ended(FlightEnd::Granted(grant)) if held => {
                self.host_tasks.nextcloud.finish(attempt);
                self.setup.nextcloud.hold(server, grant.clone());
                self.add_setup_nextcloud(grant, uses, sender);
            }
            // Granted to an attempt that was retired: nobody will use it.
            Flight::Ended(FlightEnd::Granted(grant)) => {
                if let Some(app) = &self.app {
                    app.abandon_nextcloud_sign_in(grant.0);
                }
            }
            Flight::Ended(FlightEnd::Unsure { host }) if held => {
                self.host_tasks.nextcloud.finish(attempt);
                self.setup.nextcloud_unsure(&server, host);
            }
            Flight::Ended(end) if held => {
                // Cancelled rather than finished: a browser that did not open ends the attempt
                // while its worker is still polling.
                self.host_tasks.nextcloud.cancel();
                self.setup.nextcloud_ended(end.said());
            }
            // For an attempt that was retired, which has nothing left to say.
            Flight::Opened(_) | Flight::Ended(_) => {}
        }
    }

    /// Adds the calendar and contacts account `grant` signs in to, used for `uses`, off the main
    /// thread. An account already set up for this person is offered the grant instead.
    fn add_setup_nextcloud(
        &mut self,
        grant: Grant,
        uses: Vec<AccountCapability>,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        self.setup.connecting();
        std::thread::spawn(move || {
            let result = app
                .add_nextcloud_account(grant.0.clone(), uses)
                .map(|account| account.id)
                .map_err(|error| match error {
                    MailcalError::AlreadySetUp { account_id } => ConnectFailure {
                        message: Some(l10n::account_already_set_up().to_owned()),
                        existing: Some(ExistingAccount::nextcloud(account_id, grant)),
                        ..ConnectFailure::default()
                    },
                    other => refusal(other, None, false),
                });
            sender.emit(input(SetupNextcloudInput::Added { result }));
        });
    }

    /// Asks, off the main thread, whether the server the manual pane names is Nextcloud. Until
    /// it answers the pane stands as it is.
    fn ask_setup_nextcloud(&mut self, typed: ManualForm, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let Some(server) = self.setup.adopt_manual_dav(typed) else {
            return;
        };
        std::thread::spawn(move || {
            let offered = app.offers_nextcloud_sign_in(server.clone());
            sender.emit(input(SetupNextcloudInput::Answered { server, offered }));
        });
    }
}

#[cfg(test)]
#[path = "setup_nextcloud_tests.rs"]
pub(super) mod tests;
