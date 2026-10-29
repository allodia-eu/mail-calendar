//! The shared mailboxes on an account's page (`docs/shared-mailboxes.md`): what the server
//! answered when the page opened, and switching one on or off.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use mailcal_bindings::{
    AccountCapability, AccountsSnapshot, SharedMailboxEntry, SharedMailboxSupport,
    SharedMailboxesView, Surface,
};

use super::{
    AppInput, AppModel,
    account_settings::{AccountsInput, change_failed},
    settings::notice::Notice,
};
use crate::l10n;

/// The page's shared-mailbox traffic.
#[derive(Debug)]
pub(crate) enum SharedInput {
    /// What `account`'s server answered, in the read numbered `read` ([`SharedState::read`]).
    Answered {
        account: String,
        view: Option<SharedMailboxesView>,
        read: u64,
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
    /// What each account was used for when its server was last asked.
    uses: HashMap<String, Vec<AccountCapability>>,
    /// Numbers each read of the core's answer in the order they start, on any thread.
    reads: Arc<AtomicU64>,
    /// The read each account's shown answer came from.
    shown: HashMap<String, u64>,
}

impl SharedState {
    /// What `account`'s server last answered, `None` before it has.
    pub(crate) fn view(&self, account: &str) -> Option<&SharedMailboxesView> {
        self.answers.get(account)
    }

    /// Numbers a read of the core's answer about to start, on whichever thread makes it.
    fn read(reads: &AtomicU64) -> u64 {
        reads.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Takes what the core answered for `account` in `read`, and says whether the page changed. A
    /// read that started before the one shown is older than it, and is dropped: an ask from the
    /// page opening must not undo what a switch read after it.
    fn answered(&mut self, account: &str, view: Option<SharedMailboxesView>, read: u64) -> bool {
        self.asking.remove(account);
        if read < self.shown.get(account).copied().unwrap_or(0) {
            return false;
        }
        self.shown.insert(account.to_owned(), read);
        let previous = match view {
            Some(view) => self.answers.insert(account.to_owned(), view),
            None => self.answers.remove(account),
        };
        previous.as_ref() != self.answers.get(account)
    }

    /// Whether asking `account`, now used for `uses`, again may answer something new: it was not
    /// connected yet, or could not open a shared mailbox, which a sign-in completed changes; or
    /// what it is used for changed, and switching mail or shared mailboxes off removes every one
    /// chosen. Anything else would be a request to its server on every signal.
    fn may_have_changed(&self, account: &str, uses: &[AccountCapability]) -> bool {
        !self.asking.contains(account)
            && (self.uses.get(account).is_none_or(|asked| asked != uses)
                || self.answers.get(account).is_none_or(|view| {
                    matches!(
                        view.support,
                        SharedMailboxSupport::NotConnected | SharedMailboxSupport::Unsupported
                    )
                }))
    }
}

/// What `account` is used for in `snapshot`, in the snapshot's order.
fn in_use(snapshot: &AccountsSnapshot, account: &str) -> Vec<AccountCapability> {
    snapshot
        .accounts
        .iter()
        .find(|entry| entry.id == account)
        .map(|entry| {
            entry
                .uses
                .iter()
                .filter(|use_| super::settings::account_rows::works(use_.state))
                .map(|use_| use_.capability)
                .collect()
        })
        .unwrap_or_default()
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
        let uses = in_use(&app.accounts_snapshot(), account);
        self.settings.shared.uses.insert(account.to_owned(), uses);
        let account = account.to_owned();
        let reads = Arc::clone(&self.settings.shared.reads);
        std::thread::spawn(move || {
            let read = SharedState::read(&reads);
            let view = app.shared_mailboxes(account.clone()).ok();
            sender.emit(input(SharedInput::Answered {
                account,
                view,
                read,
            }));
        });
    }

    pub(super) fn shared_input(&mut self, message: SharedInput, sender: relm4::Sender<AppInput>) {
        match message {
            SharedInput::Answered {
                account,
                view,
                read,
            } => {
                // Asked again on every signal while it may change, so only a different answer
                // redraws: a rebuild takes what the person was typing on the page.
                if self.settings.shared.answered(&account, view, read) {
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
                let reads = Arc::clone(&self.settings.shared.reads);
                // Blocking: the account is stored through the keyring, and switching a mailbox
                // off deletes what the device holds of it.
                std::thread::spawn(move || {
                    let notice = app
                        .set_shared_mailbox(account.clone(), mailbox, on)
                        .err()
                        .map(|error| change_failed(error.to_string()));
                    let read = SharedState::read(&reads);
                    let view = app.shared_mailboxes(account.clone()).ok();
                    sender.emit(input(SharedInput::Answered {
                        account,
                        view,
                        read,
                    }));
                    sender.emit(AppInput::Accounts(AccountsInput::Changed(notice)));
                });
            }
        }
    }

    /// Asks the open account page's server again when its answer may have changed: the account
    /// has connected since, or a use was switched on or off. Either is announced by one of these
    /// three signals.
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
        let Some(app) = &self.app else {
            return;
        };
        let uses = in_use(&app.accounts_snapshot(), &account);
        if self.settings.shared.may_have_changed(&account, &uses) {
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
        let reads = Arc::clone(&self.settings.shared.reads);
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
            let read = SharedState::read(&reads);
            let view = app.shared_mailboxes(account.clone()).ok();
            sender.emit(input(SharedInput::Answered {
                account,
                view,
                read,
            }));
            sender.emit(AppInput::Accounts(
                outcome.unwrap_or(AccountsInput::Changed(None)),
            ));
        });
    }
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AccountCapability, SharedMailboxSupport, SharedMailboxesView};

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
        let mail = [AccountCapability::Mail];
        let asked = |support| {
            let mut state = answered(support);
            state.uses.insert("a".to_owned(), mail.to_vec());
            state
        };
        assert!(SharedState::default().may_have_changed("a", &mail));
        assert!(asked(SharedMailboxSupport::NotConnected).may_have_changed("a", &mail));
        assert!(asked(SharedMailboxSupport::Unsupported).may_have_changed("a", &mail));
        // Listing again would be a request to the server on every signal.
        assert!(!asked(SharedMailboxSupport::Listed).may_have_changed("a", &mail));
        assert!(!asked(SharedMailboxSupport::Unreachable).may_have_changed("a", &mail));
        // Switching mail off removes every mailbox chosen, which the last answer still shows on.
        assert!(asked(SharedMailboxSupport::Listed).may_have_changed("a", &[]));
        let mut asking = asked(SharedMailboxSupport::NotConnected);
        asking.asking.insert("a".to_owned());
        assert!(!asking.may_have_changed("a", &mail), "one ask at a time");
    }

    #[test]
    fn an_answer_read_before_the_one_shown_does_not_replace_it() {
        let mut state = SharedState::default();
        let view = |selected| SharedMailboxesView {
            support: SharedMailboxSupport::Listed,
            mailboxes: vec![mailcal_bindings::SharedMailboxEntry {
                handle: "support".to_owned(),
                name: "Support".to_owned(),
                address: None,
                selected,
                listed: true,
            }],
        };
        // The page opens and asks; a switch is made and reads its answer before that ask lands.
        let ask = SharedState::read(&state.reads);
        let after_switch = SharedState::read(&state.reads);
        assert!(state.answered("a", Some(view(true)), after_switch));
        assert!(!state.answered("a", Some(view(false)), ask));
        assert_eq!(state.view("a"), Some(&view(true)));
    }
}
