//! The shared mailboxes on an account's page (`docs/shared-mailboxes.md`): what the server
//! answered when the page opened, and switching one on or off.

use std::collections::{HashMap, HashSet};

use mailcal_bindings::{SharedMailboxEntry, SharedMailboxSupport, SharedMailboxesView, Surface};

use super::{
    AppInput, AppModel,
    account_settings::{AccountsInput, change_failed},
    settings::notice::Notice,
};
use crate::l10n;

/// The page's shared-mailbox traffic.
#[derive(Debug)]
pub(crate) enum SharedInput {
    /// What `account`'s server answered.
    Answered {
        account: String,
        view: Option<SharedMailboxesView>,
    },
    /// Look up the mailbox at `address` and use `account` for it.
    Add { account: String, address: String },
    /// Use `account` for `mailbox`, or stop.
    Set {
        account: String,
        mailbox: SharedMailboxEntry,
        on: bool,
    },
}

/// What each account's page knows of its shared mailboxes. Here rather than in the window, which
/// is rebuilt on every change while the server's answer outlives several rebuilds.
#[derive(Clone, Debug, Default)]
pub(crate) struct SharedState {
    answers: HashMap<String, SharedMailboxesView>,
    /// The accounts whose server is being asked now, so a second ask does not overlap it.
    asking: HashSet<String>,
}

impl SharedState {
    /// What `account`'s server last answered, `None` before it has.
    pub(crate) fn view(&self, account: &str) -> Option<&SharedMailboxesView> {
        self.answers.get(account)
    }

    /// Whether asking `account` again may answer something new without a request to its server:
    /// it was not connected yet, or could not open a shared mailbox, which a use switched on or a
    /// sign-in completed changes.
    fn may_have_changed(&self, account: &str) -> bool {
        !self.asking.contains(account)
            && self.answers.get(account).is_none_or(|view| {
                matches!(
                    view.support,
                    SharedMailboxSupport::NotConnected | SharedMailboxSupport::Unsupported
                )
            })
    }
}

fn input(message: SharedInput) -> AppInput {
    AppInput::Accounts(AccountsInput::Shared(message))
}

impl AppModel {
    /// Asks which mailboxes `account`'s sign-in opens besides its own, off the main thread: it is a
    /// request to the server.
    pub(super) fn ask_shared_mailboxes(&mut self, account: &str, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        self.settings.shared.asking.insert(account.to_owned());
        let account = account.to_owned();
        std::thread::spawn(move || {
            let view = app.shared_mailboxes(account.clone()).ok();
            sender.emit(input(SharedInput::Answered { account, view }));
        });
    }

    pub(super) fn shared_input(&mut self, message: SharedInput, sender: relm4::Sender<AppInput>) {
        match message {
            SharedInput::Answered { account, view } => {
                self.settings.shared.asking.remove(&account);
                let previous = match view {
                    Some(view) => self.settings.shared.answers.insert(account.clone(), view),
                    None => self.settings.shared.answers.remove(&account),
                };
                // Asked again on every signal while it may change, so only a different answer
                // redraws: a rebuild takes what the person was typing on the page.
                if previous.as_ref() != self.settings.shared.answers.get(&account) {
                    self.settings.refresh_in_place();
                }
            }
            SharedInput::Add { account, address } => {
                self.add_shared_mailbox(account, address, sender);
            }
            SharedInput::Set {
                account,
                mailbox,
                on,
            } => {
                let Some(app) = self.app.clone() else {
                    return;
                };
                // Blocking: the account is stored through the keyring, and switching a mailbox
                // off deletes what the device holds of it.
                std::thread::spawn(move || {
                    let notice = app
                        .set_shared_mailbox(account.clone(), mailbox, on)
                        .err()
                        .map(|error| change_failed(error.to_string()));
                    let view = app.shared_mailboxes(account.clone()).ok();
                    sender.emit(input(SharedInput::Answered { account, view }));
                    sender.emit(AppInput::Accounts(AccountsInput::Changed(notice)));
                });
            }
        }
    }

    /// Asks the open account page's server again when its answer may have changed: the account
    /// has connected since, or its shared-mailboxes use was switched on. An account connecting is
    /// announced by one of these three signals.
    pub(super) fn ask_shared_mailboxes_again(
        &mut self,
        surface: &Surface,
        sender: relm4::Sender<AppInput>,
    ) {
        if !matches!(
            surface,
            Surface::MailboxList | Surface::Settings | Surface::Connectivity
        ) {
            return;
        }
        let Some(account) = self.settings.account.clone() else {
            return;
        };
        if self.settings.shared.may_have_changed(&account) {
            self.ask_shared_mailboxes(&account, sender);
        }
    }

    /// Asks the server for the mailbox at `address` and, when the account's sign-in opens it,
    /// uses the account for it. A refusal leaves the address where it was typed, to correct.
    fn add_shared_mailbox(
        &self,
        account: String,
        address: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        std::thread::spawn(move || {
            let outcome = match app.resolve_shared_mailbox(account.clone(), address) {
                Ok(Some(mailbox)) => app
                    .set_shared_mailbox(account.clone(), mailbox, true)
                    .err()
                    .map(|error| AccountsInput::Changed(Some(change_failed(error.to_string())))),
                Ok(None) => Some(AccountsInput::Refused(Notice::Toast(
                    l10n::settings_account_shared_not_found().to_owned(),
                ))),
                Err(error) => Some(AccountsInput::Refused(change_failed(error.to_string()))),
            };
            let view = app.shared_mailboxes(account.clone()).ok();
            sender.emit(input(SharedInput::Answered { account, view }));
            sender.emit(AppInput::Accounts(
                outcome.unwrap_or(AccountsInput::Changed(None)),
            ));
        });
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{SharedMailboxSupport, SharedMailboxesView};

    use super::SharedState;

    fn answered(support: SharedMailboxSupport) -> SharedState {
        let mut state = SharedState::default();
        state.answers.insert(
            "a".to_owned(),
            SharedMailboxesView {
                support,
                mailboxes: Vec::new(),
            },
        );
        state
    }

    #[test]
    fn a_page_asks_again_only_while_the_answer_may_still_change() {
        assert!(SharedState::default().may_have_changed("a"));
        assert!(answered(SharedMailboxSupport::NotConnected).may_have_changed("a"));
        assert!(answered(SharedMailboxSupport::Unsupported).may_have_changed("a"));
        // Listing again would be a request to the server on every signal.
        assert!(!answered(SharedMailboxSupport::Listed).may_have_changed("a"));
        assert!(!answered(SharedMailboxSupport::Unreachable).may_have_changed("a"));
        let mut asking = answered(SharedMailboxSupport::NotConnected);
        asking.asking.insert("a".to_owned());
        assert!(!asking.may_have_changed("a"), "one ask at a time");
    }
}
