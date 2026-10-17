//! "This account is already set up": an add the core refused because the mailbox is an account
//! already, and the offer to put the credential just given on that account instead
//! (`docs/accounts.md` rules 15 and 17).

use adw::prelude::*;
use mailcal_bindings::MailcalError;

use super::{
    AppInput, AppModel,
    setup_model::{AccountSubmission, ConnectFailure},
    setup_pane::{FEEDBACK, named},
    setup_state::{Phase, SetupState},
    setup_widgets::caption,
};
use crate::l10n;

/// The account an add would have duplicated, and the config that add was given, whose
/// credential can go on that account instead.
#[derive(Clone)]
pub(crate) struct ExistingAccount {
    pub(super) account_id: String,
    config_toml: String,
}

/// Never the config: it carries the password or the grant just given.
impl std::fmt::Debug for ExistingAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExistingAccount")
            .field("account_id", &self.account_id)
            .finish_non_exhaustive()
    }
}

/// The add screen's traffic about an account already set up.
#[derive(Debug)]
pub(crate) enum ExistingInput {
    /// Put the credential just given on the account already set up.
    Update,
    /// That ended, for the account it was put on.
    Updated {
        account: String,
        result: Result<(), String>,
    },
}

/// Whether a refused add of `submission` can offer its credential to the account already there:
/// an IMAP or a JMAP account, which carry a password the account can take. A calendar-only one
/// has no mailbox credential to hand over.
pub(super) fn takes_update(submission: &AccountSubmission) -> bool {
    match submission {
        AccountSubmission::Imap(form) => !form.imap_host.trim().is_empty(),
        AccountSubmission::Jmap(_) => true,
    }
}

/// What the add screen says about an add the core refused. A refused certificate comes back
/// whole, because the IMAP pane offers to accept it (`docs/certificate-exceptions.md`); its
/// rendered message carries the whole certificate, so it is deliberately not used. An account
/// already set up keeps the account it named, with `config_toml` when the route can offer it the
/// credential. Everything else is a message.
pub(super) fn refusal(
    error: MailcalError,
    config_toml: Option<String>,
    offers_exception: bool,
) -> ConnectFailure {
    match error {
        MailcalError::CertificateRejected {
            reason,
            certificate,
        } => ConnectFailure {
            message: Some(reason),
            certificate: offers_exception.then_some(certificate),
            existing: None,
        },
        other => ConnectFailure {
            message: Some(super::core_error::said(&other)),
            certificate: None,
            existing: match (other, config_toml) {
                (MailcalError::AlreadySetUp { account_id }, Some(config_toml)) => {
                    Some(ExistingAccount {
                        account_id,
                        config_toml,
                    })
                }
                _ => None,
            },
        },
    }
}

/// Adds the offer beneath the refusal in the pane on screen, when the last add was refused for an
/// account already set up. The pane names its feedback box, which a rebuild and a redraw into
/// the mounted pane have both just filled.
pub(super) fn show(window: &gtk::Window, state: &SetupState, sender: &relm4::Sender<AppInput>) {
    let Some(feedback) = state
        .existing
        .as_ref()
        .filter(|_| state.phase == Phase::Form)
        .and_then(|_| window.child())
        .and_then(|root| named(&root, FEEDBACK))
        .and_downcast::<gtk::Box>()
    else {
        return;
    };
    append_offer(&feedback, sender);
}

fn append_offer(feedback: &gtk::Box, sender: &relm4::Sender<AppInput>) {
    let update = gtk::Button::with_label(l10n::account_already_set_up_update());
    update.add_css_class("suggested-action");
    update.set_halign(gtk::Align::Start);
    let input = sender.clone();
    update.connect_clicked(move |button| {
        button.set_sensitive(false);
        input.emit(AppInput::AlreadySetUp(ExistingInput::Update));
    });
    feedback.append(&update);
    feedback.append(&caption(l10n::account_already_set_up_update_note()));
}

impl AppModel {
    pub(super) fn existing_account_input(
        &mut self,
        input: ExistingInput,
        sender: relm4::Sender<AppInput>,
    ) {
        match input {
            ExistingInput::Update => self.update_existing_account(sender),
            ExistingInput::Updated {
                account,
                result: Ok(()),
            } => {
                // Its way in may have changed, so what its page offers is asked again.
                self.settings.sign_in.forget(&account);
                self.existing_account_updated(sender);
            }
            ExistingInput::Updated {
                result: Err(message),
                ..
            } => self.setup.connect_failed(ConnectFailure {
                message: Some(l10n::status_connect_failed(&message)),
                ..ConnectFailure::default()
            }),
        }
    }

    /// Connects the account already set up with the credential the refused add carried, off the
    /// main thread: the core tests it before it stores it.
    fn update_existing_account(&mut self, sender: relm4::Sender<AppInput>) {
        let (Some(app), Some(existing)) = (self.app.clone(), self.setup.existing.clone()) else {
            return;
        };
        self.setup.connecting();
        std::thread::spawn(move || {
            let account = existing.account_id.clone();
            let result = app
                .use_credential_from(existing.account_id, existing.config_toml)
                .map_err(|error| super::core_error::said(&error));
            sender.emit(AppInput::AlreadySetUp(ExistingInput::Updated {
                account,
                result,
            }));
        });
    }
}

#[cfg(test)]
#[path = "setup_already_set_up_tests.rs"]
pub(crate) mod tests;
