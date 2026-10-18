//! A sign-in of its own for a mailbox's calendar and contacts, when the mailbox's sign-in does
//! not reach their server (`docs/accounts.md` rule 18, `docs/onboarding.md`): setup's step after
//! the add, and the form the account's page opens. Either connects them as an account of their
//! own linked to the mailbox (`add_linked_calendar_account`), a blocking call off the main
//! thread. The widgets are [`super::separate_sign_in_view`].

use mailcal_bindings::{AccountCapability, SeparateServer};

use super::{
    AppInput, AppModel, account_settings::AccountsInput, account_sign_in::Password,
    setup_state::Phase,
};

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
    Open {
        account: String,
        server: SeparateServer,
    },
    /// The page's form closed without connecting.
    Close,
    /// Connect with what was typed.
    Connect {
        from: SeparateFrom,
        username: String,
        password: Password,
    },
    /// That connect answered, for the mailbox it was asked for.
    Connected {
        from: SeparateFrom,
        account: String,
        result: Result<(), String>,
    },
    /// Setup's step was skipped: the mailbox stops being used for what needs the sign-in.
    Skip,
    /// Those uses are off.
    Skipped { account: String },
}

/// The form for one mailbox: its server, the login last typed, and how its connect stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeparateForm {
    /// Which form this is, so a window drawing it can tell a new one from a redraw.
    pub(super) id: u64,
    pub(super) account: String,
    pub(super) server: SeparateServer,
    pub(super) username: String,
    /// A connect is running, which holds every control.
    pub(super) busy: bool,
    /// Why the last connect failed, said under the fields.
    pub(super) error: Option<String>,
    /// The account whose link step comes after setup's step.
    pub(super) next: String,
}

impl SeparateForm {
    fn new(id: u64, account: String, server: SeparateServer, next: String) -> Self {
        Self {
            id,
            username: server.username.clone(),
            account,
            server,
            busy: false,
            error: None,
            next,
        }
    }

    fn connecting(&mut self, username: String) {
        self.username = username;
        self.busy = true;
        self.error = None;
    }

    fn failed(&mut self, error: String) {
        self.busy = false;
        self.error = Some(error);
    }
}

/// What Skip switches: each use the server is needed for, off, and nothing else.
pub(super) fn skip_changes(server: &SeparateServer) -> Vec<(AccountCapability, bool)> {
    server.uses.iter().map(|&use_| (use_, false)).collect()
}

/// The server `account`'s calendar and contacts need a sign-in of their own at, read from the
/// accounts the core lists.
pub(super) fn server_of(
    snapshot: &mailcal_bindings::AccountsSnapshot,
    account: &str,
) -> Option<SeparateServer> {
    snapshot
        .accounts
        .iter()
        .find(|entry| entry.id == account)?
        .separate_server
        .clone()
}

impl super::setup_state::SetupState {
    /// Holds the step for `account`, just added, until its link step would be shown.
    pub(super) fn hold_separate(&mut self, account: &str, server: SeparateServer) {
        self.separate = Some(SeparateForm::new(
            0,
            account.to_owned(),
            server,
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
    pub(super) fn separate_input(&mut self, input: SeparateInput, sender: relm4::Sender<AppInput>) {
        match input {
            SeparateInput::Open { account, server } => {
                self.settings.separate_forms = self.settings.separate_forms.wrapping_add(1);
                let id = self.settings.separate_forms;
                self.settings.separate =
                    Some(SeparateForm::new(id, account, server, String::new()));
            }
            SeparateInput::Close => self.settings.separate = None,
            SeparateInput::Connect {
                from,
                username,
                password,
            } => self.connect_separate(from, username, password, sender),
            SeparateInput::Connected {
                from,
                account,
                result,
            } => self.separate_connected(from, &account, result, sender),
            SeparateInput::Skip => self.skip_separate(sender),
            SeparateInput::Skipped { account } => {
                if self
                    .setup
                    .separate
                    .as_ref()
                    .map(|step| step.account.as_str())
                    == Some(account.as_str())
                {
                    self.leave_separate_step(sender);
                }
            }
        }
    }

    fn connect_separate(
        &mut self,
        from: SeparateFrom,
        username: String,
        password: Password,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let form = match from {
            SeparateFrom::Setup => self.setup.separate_step(),
            SeparateFrom::Page => self.settings.separate.as_mut(),
        };
        let Some(form) = form.filter(|form| !form.busy) else {
            return;
        };
        form.connecting(username);
        let (account, uses, username) = (
            form.account.clone(),
            form.server.uses.clone(),
            form.username.clone(),
        );
        if from == SeparateFrom::Setup {
            self.setup.bump();
        }
        std::thread::spawn(move || {
            let result = app
                .add_linked_calendar_account(account.clone(), uses, username, password.0)
                .map(|_| ())
                .map_err(|error| super::core_error::said(&error));
            sender.emit(AppInput::Accounts(AccountsInput::Separate(
                SeparateInput::Connected {
                    from,
                    account,
                    result,
                },
            )));
        });
    }

    fn separate_connected(
        &mut self,
        from: SeparateFrom,
        account: &str,
        result: Result<(), String>,
        sender: relm4::Sender<AppInput>,
    ) {
        if result.is_ok() {
            if let Some(app) = &self.app {
                self.snapshot = app.mailbox_list();
                self.connectivity =
                    super::connectivity::ConnectivityState::pull(app, &self.snapshot.accounts);
            }
            self.sync_after_account_change(sender.clone());
        }
        let form = match from {
            SeparateFrom::Setup => self.setup.separate_step(),
            SeparateFrom::Page => self.settings.separate.as_mut(),
        };
        let Some(form) = form.filter(|form| form.account == account) else {
            return;
        };
        match (from, result) {
            (SeparateFrom::Setup, Ok(())) => self.leave_separate_step(sender),
            (SeparateFrom::Page, Ok(())) => {
                self.settings.separate = None;
                self.settings.refresh_in_place();
            }
            (from, Err(error)) => {
                form.failed(error);
                if from == SeparateFrom::Setup {
                    self.setup.bump();
                }
            }
        }
    }

    /// Switches off what the step was for, off the main thread: it deletes what the device holds
    /// of those uses.
    fn skip_separate(&mut self, sender: relm4::Sender<AppInput>) {
        let Some(step) = self.setup.separate_step().filter(|step| !step.busy) else {
            return;
        };
        step.busy = true;
        let (account, changes) = (step.account.clone(), skip_changes(&step.server));
        self.setup.bump();
        let Some(app) = self.app.clone() else {
            return self.leave_separate_step(sender);
        };
        std::thread::spawn(move || {
            for (capability, on) in changes {
                if let Err(error) = app.set_account_capability(account.clone(), capability, on) {
                    log::warn!("setup: a use could not be switched off ({error})");
                }
            }
            sender.emit(AppInput::Accounts(AccountsInput::Separate(
                SeparateInput::Skipped { account },
            )));
        });
    }

    /// On from setup's step, when it is on screen, to the link step that comes after it.
    pub(super) fn leave_separate_step(&mut self, sender: relm4::Sender<AppInput>) {
        let Some((account, next)) = self.setup.leave_separate() else {
            return;
        };
        if let Some(app) = &self.app {
            self.snapshot = app.mailbox_list();
        }
        self.onward(&account, &next, sender);
    }
}

#[cfg(test)]
#[path = "separate_sign_in_tests.rs"]
pub(crate) mod tests;
