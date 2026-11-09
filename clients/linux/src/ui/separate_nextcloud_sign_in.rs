//! A separate calendar server that is Nextcloud signs in with Nextcloud, from the form that
//! otherwise asks for its password (`docs/nextcloud-sign-in.md` rule 2). The form's stages are
//! the provider sign-in's, so it draws the same way: starting while the core asks the server,
//! waiting while the browser holds it, finishing while the core adds the account. The browser
//! half is [`super::super::nextcloud_flight`]'s.
//!
//! A grant the core could not make an account of stays on the form, and the next press tries it
//! again without the browser. Leaving the form withdraws it.

use super::{ProviderInput, SignInEnd, other, provider};
use crate::ui::{
    AppInput, AppModel,
    nextcloud_flight::{self, Flight, FlightEnd, Grant},
    separate_sign_in::SeparateFrom,
};

impl AppModel {
    /// Takes the form `from` names off its sign-in, when one is out, and releases the slot. A
    /// grant it kept is withdrawn, unless the core is making an account of it now.
    pub(in crate::ui) fn retire_provider_sign_in(&mut self, from: SeparateFrom) {
        let Some(form) = self.separate_form(from) else {
            return;
        };
        let retired = form.retire_sign_in();
        let finishing = matches!(form.stage, super::SignInStage::Finishing(_));
        let kept = if finishing { None } else { form.grant.take() };
        if retired {
            self.host_tasks.separate.cancel();
        }
        if let (Some(grant), Some(app)) = (kept, &self.app) {
            app.abandon_nextcloud_sign_in(grant.0);
        }
    }

    /// Starts signing the form's server in with Nextcloud, superseding the other form's attempt.
    pub(super) fn start_separate_nextcloud(
        &mut self,
        from: SeparateFrom,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if self.separate_form(from).is_none_or(|form| form.held()) {
            return;
        }
        self.retire_provider_sign_in(other(from));
        let (attempt, cancel) = self.host_tasks.separate.start();
        let Some(form) = self
            .separate_form(from)
            .and_then(|form| form.begin_sign_in(attempt).then_some(form))
        else {
            return;
        };
        let (account, uses) = (form.account.clone(), form.section.uses.clone());
        if let Some(grant) = form.grant.clone() {
            form.sign_in_prepared(attempt, Ok(()));
            form.sign_in_redirected(attempt);
            return self.add_separate_nextcloud(from, attempt, grant, sender);
        }
        self.redraw_separate(from);
        let reported = account.clone();
        nextcloud_flight::fly(
            app,
            cancel,
            move |app| app.begin_separate_server_nextcloud_sign_in(account, uses),
            move |flight| {
                sender.emit(provider(
                    from,
                    ProviderInput::Nextcloud {
                        attempt,
                        account: reported.clone(),
                        flight,
                    },
                ));
            },
        );
    }

    pub(super) fn separate_nextcloud_flight(
        &mut self,
        from: SeparateFrom,
        attempt: u64,
        account: String,
        flight: Flight,
        sender: relm4::Sender<AppInput>,
    ) {
        let held = self.host_tasks.separate.holds(attempt);
        let form = self
            .separate_form(from)
            .filter(|form| held && form.account == account);
        match flight {
            Flight::Opened(login_url) => {
                if form.is_some_and(|form| form.sign_in_prepared(attempt, Ok(()))) {
                    nextcloud_flight::open_browser(&login_url, move || {
                        sender.emit(provider(
                            from,
                            ProviderInput::Ended {
                                attempt,
                                account,
                                end: SignInEnd::Failed,
                            },
                        ));
                    });
                    self.redraw_separate(from);
                }
            }
            Flight::Ended(FlightEnd::Granted(grant)) => {
                match form.and_then(|form| form.sign_in_redirected(attempt).then_some(form)) {
                    Some(form) => {
                        form.grant = Some(grant.clone());
                        self.add_separate_nextcloud(from, attempt, grant, sender);
                    }
                    // Granted to an attempt the form has left: nobody will use it.
                    None => {
                        if let Some(app) = &self.app {
                            app.abandon_nextcloud_sign_in(grant.0);
                        }
                    }
                }
            }
            Flight::Ended(end) => {
                let end = end
                    .said()
                    .map_or(SignInEnd::Cancelled, |said| SignInEnd::Done(Err(said)));
                self.provider_ended(from, attempt, &account, end, sender);
            }
        }
    }

    /// Makes the form's account of `grant` off the main thread; the form keeps the grant until
    /// that has worked.
    fn add_separate_nextcloud(
        &mut self,
        from: SeparateFrom,
        attempt: u64,
        grant: Grant,
        sender: relm4::Sender<AppInput>,
    ) {
        let (Some(app), Some(form)) = (self.app.clone(), self.separate_form(from)) else {
            return;
        };
        let (account, uses) = (form.account.clone(), form.section.uses.clone());
        self.host_tasks.separate.finish(attempt);
        self.redraw_separate(from);
        std::thread::spawn(move || {
            let result = app
                .add_linked_nextcloud_account(account.clone(), uses, grant.0)
                .map(|_| ())
                .map_err(|error| super::super::core_error::said(&error));
            sender.emit(provider(
                from,
                ProviderInput::NextcloudAdded {
                    attempt,
                    account,
                    result,
                },
            ));
        });
    }

    pub(super) fn separate_nextcloud_added(
        &mut self,
        from: SeparateFrom,
        attempt: u64,
        account: &str,
        result: Result<(), String>,
        sender: relm4::Sender<AppInput>,
    ) {
        if result.is_ok()
            && let Some(form) = self
                .separate_form(from)
                .filter(|form| form.account == account)
        {
            form.grant = None;
        }
        self.provider_ended(from, attempt, account, SignInEnd::Done(result), sender);
    }
}
