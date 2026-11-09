//! A sign-in of its own for a mailbox's calendar and contacts, when the mailbox's sign-in does
//! not reach their server (`docs/accounts.md` rule 18, `docs/onboarding.md`): setup's step after
//! the add, and the form the account's page opens. Both draw the core's section
//! (`AccountPage::separate_server`) and send Connect and Skip to the core as changes on the page;
//! the form ends once the core draws no section, whichever way the sign-in was given. Signing in
//! at the server's own provider, or with Nextcloud, is still run here
//! ([`super::separate_server_sign_in`]). The widgets are [`super::separate_sign_in_view`].

use mailcal_bindings::{
    AccountPageIntent, AccountSeparateServer, AccountSeparateSignIn, Intent, TypedPassword,
};

use super::{AppInput, AppModel, separate_server_sign_in::SignInStage, setup_state::Phase};

/// A password on its way to the core. Never printed: an input can reach a log through `Debug`.
pub(crate) struct Password(pub(crate) String);

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(..)")
    }
}

/// Where a sign-in of its own was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeparateFrom {
    /// Setup's step, after the mailbox was added.
    Setup,
    /// The form on the account's page.
    Page,
}

/// The traffic of both, as one input of the account pages'.
#[derive(Debug)]
pub(crate) enum SeparateInput {
    /// Open the account page's form for `account`.
    Open { account: String },
    /// The page's form closed without connecting.
    Close,
    /// Connect with what was typed.
    Connect {
        from: SeparateFrom,
        username: String,
        password: Password,
    },
    /// Setup's step was skipped: the mailbox stops being used for what needs the sign-in.
    Skip,
    /// The sign-in at the server's own provider, for the form `from` names.
    Provider {
        from: SeparateFrom,
        input: super::separate_server_sign_in::ProviderInput,
    },
}

/// The form for one mailbox: the core's section, and the browser sign-in this client runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeparateForm {
    /// Which form this is, so a window drawing it can tell a new one from a redraw.
    pub(super) id: u64,
    pub(super) account: String,
    /// The section as the core last drew it.
    pub(super) section: AccountSeparateServer,
    /// A Nextcloud grant no account was made from yet, kept so the next press tries it again
    /// without the browser; the server hands it out once.
    pub(super) grant: Option<super::nextcloud_flight::Grant>,
    /// Where that sign-in stands.
    pub(super) stage: SignInStage,
    /// Why the last sign-in in the browser failed; a Connect's failure is the section's.
    pub(super) error: Option<String>,
    /// The account whose link step comes after setup's step.
    pub(super) next: String,
}

impl SeparateForm {
    pub(super) fn new(
        id: u64,
        account: String,
        section: AccountSeparateServer,
        next: String,
    ) -> Self {
        Self {
            id,
            account,
            section,
            grant: None,
            stage: SignInStage::Idle,
            error: None,
            next,
        }
    }

    pub(super) fn failed(&mut self, error: String) {
        self.error = Some(error);
    }

    /// What the form says under its fields: the browser sign-in's failure, else the core's.
    pub(super) fn said(&self) -> Option<String> {
        self.error.clone().or_else(|| {
            self.section
                .failed
                .as_ref()
                .map(super::core_error::refusal_said)
        })
    }

    /// The sign-in the server offers in the browser, when it has said so.
    pub(super) const fn offers(&self) -> Option<AccountSeparateSignIn> {
        self.section.sign_in
    }
}

impl super::setup_state::SetupState {
    /// Holds the step for `account`, just added, until its link step would be shown.
    pub(super) fn hold_separate(&mut self, account: &str, section: AccountSeparateServer) {
        self.separate = Some(SeparateForm::new(
            0,
            account.to_owned(),
            section,
            String::new(),
        ));
    }

    /// Shows the step held for `added` in place of `next`'s link step, which comes after it.
    /// `false` when none is held, or it is on screen already.
    pub(super) fn show_separate(&mut self, added: &str, next: &str) -> bool {
        let Some(step) = self
            .separate
            .as_mut()
            .filter(|step| step.account == added && self.phase != Phase::Separate)
        else {
            return false;
        };
        next.clone_into(&mut step.next);
        self.phase = Phase::Separate;
        self.error = None;
        self.bump();
        true
    }

    /// The step on screen, for a change of its own.
    fn separate_step(&mut self) -> Option<&mut SeparateForm> {
        (self.visible && self.phase == Phase::Separate)
            .then_some(self.separate.as_mut())
            .flatten()
    }

    /// Takes the step off screen: the mailbox and the link step that come next.
    pub(super) fn leave_separate(&mut self) -> Option<(String, String)> {
        if self.phase != Phase::Separate {
            return None;
        }
        self.separate.take().map(|step| (step.account, step.next))
    }
}

impl AppModel {
    /// The form `from` names, while it is on screen.
    pub(super) fn separate_form(&mut self, from: SeparateFrom) -> Option<&mut SeparateForm> {
        match from {
            SeparateFrom::Setup => self.setup.separate_step(),
            SeparateFrom::Page => self.settings.separate.as_mut(),
        }
    }

    /// Redraws the form `from` names in place, so what was typed into it stands. The page's
    /// form follows the model on every redraw already.
    pub(super) fn redraw_separate(&mut self, from: SeparateFrom) {
        if from == SeparateFrom::Setup {
            self.setup.report();
        }
    }

    /// The section `account`'s page draws, or `None` once nothing needs the sign-in.
    fn separate_section(&self, account: &str) -> Option<AccountSeparateServer> {
        self.app
            .as_ref()?
            .account_page(account.to_owned())?
            .separate_server
    }

    pub(super) fn separate_input(&mut self, input: SeparateInput, sender: relm4::Sender<AppInput>) {
        match input {
            SeparateInput::Open { account } => {
                self.retire_provider_sign_in(SeparateFrom::Page);
                let Some(section) = self.separate_section(&account) else {
                    return;
                };
                self.settings.separate_forms = self.settings.separate_forms.wrapping_add(1);
                let id = self.settings.separate_forms;
                self.settings.separate =
                    Some(SeparateForm::new(id, account, section, String::new()));
            }
            SeparateInput::Close => {
                self.retire_provider_sign_in(SeparateFrom::Page);
                self.settings.separate = None;
            }
            SeparateInput::Provider { from, input } => self.provider_input(from, input, sender),
            SeparateInput::Connect {
                from,
                username,
                password,
            } => self.send_separate(from, |account| AccountPageIntent::ConnectSeparateServer {
                account,
                username,
                password: TypedPassword { text: password.0 },
            }),
            SeparateInput::Skip => self.send_separate(SeparateFrom::Setup, |account| {
                AccountPageIntent::StopUsingSeparateServer { account }
            }),
        }
    }

    /// Sends the form's Connect or Skip to the core, which draws it running on the section.
    fn send_separate(
        &mut self,
        from: SeparateFrom,
        change: impl FnOnce(String) -> AccountPageIntent,
    ) {
        if self.separate_form(from).is_none_or(|form| form.held()) {
            return;
        }
        // A password typed while the browser holds the sign-in is the person's answer instead.
        self.retire_provider_sign_in(from);
        let Some(form) = self.separate_form(from) else {
            return;
        };
        form.error = None;
        let intent = change(form.account.clone());
        self.dispatch(Intent::AccountPage { intent });
    }

    /// Draws each open form again from the core, after a signal that may have moved it. One the
    /// core draws no section for is over: the sign-in was given, or the uses switched off.
    pub(super) fn separate_signalled(&mut self, sender: &relm4::Sender<AppInput>) {
        for from in [SeparateFrom::Setup, SeparateFrom::Page] {
            self.separate_redrawn(from, sender);
        }
    }

    /// Draws the form `from` names again from the core, and goes on from it once it is over.
    pub(super) fn separate_redrawn(
        &mut self,
        from: SeparateFrom,
        sender: &relm4::Sender<AppInput>,
    ) {
        let Some(account) = self.separate_form(from).map(|form| form.account.clone()) else {
            return;
        };
        let section = self.separate_section(&account);
        let Some(form) = self.separate_form(from) else {
            return;
        };
        match section {
            Some(section) if section == form.section => {}
            Some(section) => {
                form.section = section;
                self.redraw_separate(from);
            }
            None => self.separate_over(from, sender.clone()),
        }
    }

    /// The form `from` names is over: setup goes on to its link step and the page's form closes.
    /// The mailbox gave its uses to an account of their own, or stopped using them, so the
    /// accounts are read again.
    fn separate_over(&mut self, from: SeparateFrom, sender: relm4::Sender<AppInput>) {
        self.accounts_changed(sender.clone());
        match from {
            SeparateFrom::Setup => self.leave_separate_step(sender),
            SeparateFrom::Page => {
                self.retire_provider_sign_in(from);
                self.settings.separate = None;
                self.settings.refresh_in_place();
            }
        }
    }

    /// The mailbox gave uses to an account of their own: the accounts are read again.
    pub(super) fn accounts_changed(&mut self, sender: relm4::Sender<AppInput>) {
        if let Some(app) = &self.app {
            self.snapshot.replace(app.mailbox_list());
        }
        self.pull_notices();
        self.sync_after_account_change(sender);
    }

    /// On from setup's step, when it is on screen, to the link step that comes after it.
    pub(super) fn leave_separate_step(&mut self, sender: relm4::Sender<AppInput>) {
        self.retire_provider_sign_in(SeparateFrom::Setup);
        let Some((account, next)) = self.setup.leave_separate() else {
            return;
        };
        if let Some(app) = &self.app {
            self.snapshot.replace(app.mailbox_list());
        }
        self.onward(&account, &next, sender);
    }
}

#[cfg(test)]
#[path = "separate_sign_in_tests.rs"]
pub(crate) mod tests;
