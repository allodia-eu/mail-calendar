//! Signing an existing Microsoft or Google account in again, through the core's consent route:
//! for what it is used for, after an expired sign-in or a withheld permission, or with a use
//! added when a switch on its Settings page asks for one the grant lacks. The account keeps its
//! id, settings and downloaded mail (`docs/provider-oauth.md` rule 11).

use mailcal_bindings::AccountCapability;

use super::{
    AppInput, AppModel,
    account_settings::AccountsInput,
    oauth_loopback::{self, CallbackOutcome, OAuthLoopback},
    settings::notice::Notice,
};
use crate::l10n;

/// Where a sign-in was asked for, which is where its outcome is said: on the account's page, or
/// in the main window's notice, beside the banner that asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsentFrom {
    Settings,
    MainWindow,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConsentOutcome {
    SignedIn,
    Cancelled,
    Failed(String),
}

/// One sign-in's traffic back to the model.
#[derive(Debug)]
pub(crate) struct ConsentFinished {
    pub(crate) attempt: u64,
    pub(crate) from: ConsentFrom,
    pub(crate) outcome: ConsentOutcome,
}

/// What the account's page says once a sign-in has ended. A cancelled one says nothing: the
/// person closed it.
pub(super) fn settings_notice(outcome: &ConsentOutcome) -> Option<Notice> {
    match outcome {
        ConsentOutcome::SignedIn => {
            Some(Notice::Toast(l10n::settings_account_signed_in().to_owned()))
        }
        ConsentOutcome::Cancelled => None,
        ConsentOutcome::Failed(error) => Some(Notice::Error {
            title: l10n::settings_account_signin_failed().to_owned(),
            detail: error.clone(),
        }),
    }
}

impl AppModel {
    /// Opens the provider's sign-in for `account` in the browser, asking for what it is used for
    /// plus `adding`, and completes it off the main thread. A second request replaces the first.
    pub(super) fn start_account_consent(
        &mut self,
        account: String,
        adding: Vec<AccountCapability>,
        from: ConsentFrom,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let (attempt, cancel) = self.host_tasks.consent.start();
        let finished = move |outcome| {
            AppInput::Accounts(AccountsInput::Consented(ConsentFinished {
                attempt,
                from,
                outcome,
            }))
        };
        let begun = OAuthLoopback::bind()
            .map_err(|_| l10n::account_signin_browser_failed().to_owned())
            .and_then(|loopback| {
                app.begin_account_consent(account.clone(), loopback.redirect_uri(), adding)
                    .map(|start| (loopback, start))
                    .map_err(|error| error.to_string())
            });
        let (loopback, start) = match begun {
            Ok(begun) => begun,
            Err(error) => {
                sender.emit(finished(ConsentOutcome::Failed(error)));
                return;
            }
        };
        if from == ConsentFrom::Settings {
            let address = app
                .accounts_snapshot()
                .accounts
                .into_iter()
                .find(|entry| entry.id == account)
                .map_or(account, |entry| entry.address);
            self.settings
                .notify(Notice::Toast(l10n::settings_account_signin_waiting(
                    &address,
                )));
        }
        let failed = sender.clone();
        oauth_loopback::launch_browser(&start.authorization_url, move || {
            failed.emit(finished(ConsentOutcome::Failed(
                l10n::account_signin_browser_failed().to_owned(),
            )));
        });
        std::thread::spawn(move || {
            let outcome = match loopback.wait(
                &cancel,
                l10n::account_signin_timeout(),
                l10n::account_signin_browser_failed(),
            ) {
                CallbackOutcome::Received(callback_url) => {
                    match app.complete_account_consent(start.pending, callback_url) {
                        Ok(()) => ConsentOutcome::SignedIn,
                        Err(error) => ConsentOutcome::Failed(error.to_string()),
                    }
                }
                CallbackOutcome::Cancelled => ConsentOutcome::Cancelled,
                CallbackOutcome::Failed(error) => ConsentOutcome::Failed(error),
            };
            sender.emit(finished(outcome));
        });
    }

    /// A sign-in ended. The core has already installed the new grant and reconciled the account's
    /// prompts; this says so where it was asked for, and repaints what the grant changed.
    pub(super) fn account_consent_finished(&mut self, finished: &ConsentFinished) {
        if !self.host_tasks.consent.finish(finished.attempt) {
            return;
        }
        if let Some(app) = &self.app {
            self.snapshot = app.mailbox_list();
            self.connectivity =
                super::connectivity::ConnectivityState::pull(app, &self.snapshot.accounts);
        }
        match finished.from {
            ConsentFrom::Settings => match settings_notice(&finished.outcome) {
                Some(notice) => self.settings.notify(notice),
                None => self.settings.refresh_in_place(),
            },
            ConsentFrom::MainWindow => {
                self.notice = matches!(finished.outcome, ConsentOutcome::Failed(_))
                    .then(|| l10n::signin_expired_failed().to_owned());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConsentOutcome, Notice, settings_notice};
    use crate::l10n;

    #[test]
    fn the_page_says_how_a_sign_in_ended_unless_the_person_closed_it() {
        assert_eq!(
            settings_notice(&ConsentOutcome::SignedIn),
            Some(Notice::Toast(l10n::settings_account_signed_in().to_owned()))
        );
        assert_eq!(settings_notice(&ConsentOutcome::Cancelled), None);
        // A failure is a dialog to dismiss, not a line the person may have scrolled past.
        assert_eq!(
            settings_notice(&ConsentOutcome::Failed("refused".to_owned())),
            Some(Notice::Error {
                title: l10n::settings_account_signin_failed().to_owned(),
                detail: "refused".to_owned(),
            })
        );
    }
}
