//! Signing a calendar and contacts account in with Nextcloud from its page or from the notice of
//! a refused sign-in: again, for one a Nextcloud sign-in made, or instead of its typed password
//! (`docs/nextcloud-sign-in.md` rules 2 and 7). The browser half is
//! [`super::super::nextcloud_flight`]'s; this is the account's half, in the shape of the
//! provider sign-in beside it, so its page draws both the same way.
//!
//! The server hands a grant out once, so one the core could not put on the account is kept, for
//! that account, and the next press tries it again without the browser. It is withdrawn when the
//! person leaves the page or signs another account in.

use mailcal_bindings::{MailcalApp, MailcalError};

use super::{Attempt, Outcome, SignInInput, Switch, input};
use crate::{
    l10n,
    ui::{
        AppInput, AppModel,
        account_consent::ConsentFrom,
        nextcloud_flight::{self, Flight, FlightEnd, Grant},
    },
};

impl AppModel {
    /// Whether `account` signs in with Nextcloud: a Nextcloud sign-in made it, or its server
    /// has answered that it is Nextcloud.
    pub(super) fn signs_in_with_nextcloud(&self, app: &MailcalApp, account: &str) -> bool {
        self.settings
            .sign_in
            .options(account)
            .is_some_and(|options| options.nextcloud)
            || app
                .accounts_snapshot()
                .accounts
                .iter()
                .any(|entry| entry.id == account && entry.nextcloud)
    }

    /// Starts signing `account` in with Nextcloud, replacing any other change. A grant kept from
    /// the last try goes straight to the core.
    pub(super) fn start_account_nextcloud_sign_in(
        &mut self,
        account: String,
        switch: Switch,
        from: ConsentFrom,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let (id, cancel) = self.host_tasks.account_sign_in.start();
        let attempt = Attempt {
            id,
            account: account.clone(),
            from,
            switch,
        };
        self.settings.sign_in.question = None;
        self.settings.sign_in.begin(&account);
        if let Some(grant) = self.settings.sign_in.held_grant(&account) {
            return self.complete_account_nextcloud(attempt, grant, sender);
        }
        self.abandon_held_nextcloud_grant();
        self.say_waiting(&app, &account, from);
        self.settings.refresh_in_place();
        nextcloud_flight::fly(
            app,
            cancel,
            move |app| app.begin_account_nextcloud_sign_in(account),
            move |flight| {
                sender.emit(input(SignInInput::Nextcloud {
                    attempt: attempt.clone(),
                    flight,
                }));
            },
        );
    }

    pub(super) fn account_nextcloud_flight(
        &mut self,
        attempt: Attempt,
        flight: Flight,
        sender: relm4::Sender<AppInput>,
    ) {
        let held = self.host_tasks.account_sign_in.holds(attempt.id);
        match flight {
            Flight::Opened(login_url) if held => {
                let (failed, abandoned) = (sender, attempt.clone());
                nextcloud_flight::open_browser(&login_url, move || {
                    failed.emit(input(SignInInput::Nextcloud {
                        attempt: abandoned,
                        flight: Flight::Ended(FlightEnd::Failed(
                            l10n::account_signin_browser_failed().to_owned(),
                        )),
                    }));
                });
                self.settings.sign_in.in_browser(&attempt.account);
                self.settings.refresh_in_place();
            }
            Flight::Opened(_) => {}
            Flight::Ended(FlightEnd::Granted(grant)) if held => {
                self.complete_account_nextcloud(attempt, grant, sender);
            }
            // Granted to an attempt that was replaced: nobody will use it.
            Flight::Ended(FlightEnd::Granted(grant)) => {
                if let Some(app) = &self.app {
                    app.abandon_nextcloud_sign_in(grant.0);
                }
            }
            Flight::Ended(end) => {
                // Stops the worker too: a browser that did not open ends the attempt while it is
                // still polling.
                if let Some(cancel) = self.host_tasks.account_sign_in.cancel_token(attempt.id) {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                }
                let outcome = end.said().map_or(Outcome::Cancelled, Outcome::Failed);
                self.account_sign_in_finished(&attempt, &outcome, sender);
            }
        }
    }

    /// Puts `grant` on the account off the main thread, keeping it until the core has.
    fn complete_account_nextcloud(
        &mut self,
        attempt: Attempt,
        grant: Grant,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        self.settings.sign_in.completing(&attempt.account);
        self.settings
            .sign_in
            .hold_grant(attempt.account.clone(), grant.clone());
        self.settings.refresh_in_place();
        std::thread::spawn(move || {
            let result = app.complete_account_nextcloud_sign_in(attempt.account.clone(), grant.0);
            // A grant for somebody else has been withdrawn by the core already.
            let spent = matches!(result, Ok(()) | Err(MailcalError::DifferentAccount));
            let outcome = match result {
                Ok(()) => Outcome::Switched,
                Err(error) => Outcome::Failed(super::super::core_error::said(&error)),
            };
            sender.emit(input(SignInInput::NextcloudCompleted {
                attempt,
                outcome,
                spent,
            }));
        });
    }

    pub(super) fn account_nextcloud_completed(
        &mut self,
        attempt: &Attempt,
        outcome: &Outcome,
        spent: bool,
        sender: relm4::Sender<AppInput>,
    ) {
        if spent {
            self.settings.sign_in.take_grant(&attempt.account);
        }
        self.account_sign_in_finished(attempt, outcome, sender);
    }

    /// Withdraws the grant kept from a sign-in that did not go through, when there is one: its
    /// page was left, or another account is signing in.
    pub(in crate::ui) fn abandon_held_nextcloud_grant(&mut self) {
        if let (Some(grant), Some(app)) = (self.settings.sign_in.take_any_grant(), &self.app) {
            app.abandon_nextcloud_sign_in(grant.0);
        }
    }
}
