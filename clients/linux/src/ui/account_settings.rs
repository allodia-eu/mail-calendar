//! The traffic of Settings → Accounts' account pages: which page is open, and the changes made on
//! one. A use, a link and a removal are the core's to run (`docs/waiting.md`): dispatched, they
//! return at once, and the page says they run. Every other change is a core call that may touch
//! the keyring or a server, so it runs off the main thread and redraws the page when it is done.

use mailcal_bindings::{
    AccountCapability, AccountChangeNoticeRaised, AccountPageIntent, Intent, LinkSlot,
};

use super::{
    AppInput, AppModel,
    account_consent::{ConsentFinished, ConsentFrom},
    settings::notice::Notice,
};
use crate::l10n;

/// One input rather than several, so the dispatch has a single arm: they are one page's traffic
/// and each ends in the same redraw.
#[derive(Debug)]
pub(crate) enum AccountsInput {
    /// Open an account's page, or go back to the list.
    Show(Option<String>),
    /// Move the open Settings to an account's page under Accounts, from another category.
    Visit(String),
    /// A use, a link or a removal on an account's page, for the core to run.
    Change(AccountPageIntent),
    /// The links setup's link step chose for an account it just added, made one after another.
    LinkFromSetup {
        account: String,
        links: Vec<(LinkSlot, String)>,
    },
    /// Sign the account in again at its provider, asking for what it is used for plus `adding`.
    SignInAgain {
        account: String,
        adding: Vec<AccountCapability>,
    },
    /// A sign-in asked for here or by a banner ended.
    Consented(ConsentFinished),
    /// A change finished, with what the page should say about it, if anything.
    Changed(Option<Notice>),
    /// A change was refused: say so, and leave the page as the person left it.
    Refused(Notice),
    /// The Sign-in section of an IMAP or JMAP account (`super::account_sign_in`).
    SignIn(super::account_sign_in::SignInInput),
    /// A sign-in of its own for a mailbox's calendar and contacts, from its page or from setup
    /// (`super::separate_sign_in`).
    Separate(super::separate_sign_in::SeparateInput),
    /// Allow the server found on another site for an account's calendar or contacts
    /// (`super::server_change`).
    AllowServerChange {
        account: String,
        review: super::server_change::Review,
        from: super::server_change::ReviewFrom,
    },
    /// A lookup replaced the question while it was shown, so the allow moved nothing: ask the
    /// new one.
    ServerChangeReplaced {
        account: String,
        review: super::server_change::Review,
        from: super::server_change::ReviewFrom,
    },
    /// Allowing it failed, asked from outside Settings: the account's page says so.
    ServerChangeFailed(String),
}

/// A change the account's page could not make, and why.
pub(super) fn change_failed(detail: String) -> Notice {
    Notice::Error {
        title: l10n::settings_account_change_failed_title().to_owned(),
        detail,
    }
}

/// Whether no account is set up at all, mail or not, which is when setup is the first-run one
/// the person cannot close (`docs/accounts.md` rule 11). The mailbox snapshot cannot say: it
/// lists the accounts used for mail, and an account for a calendar and contacts alone is in none.
pub(super) fn no_accounts(app: Option<&mailcal_bindings::MailcalApp>) -> bool {
    app.is_none_or(|app| app.accounts_snapshot().accounts.is_empty())
}

impl AppModel {
    pub(super) fn has_no_accounts(&self) -> bool {
        no_accounts(self.app.as_deref())
    }

    /// An account was removed. Removing the last one returns to first-run setup, which Settings
    /// gives way to; otherwise Settings goes back to its list, since the page that asked is the
    /// account that is gone.
    pub(super) fn account_removed(&mut self) {
        if let Some(app) = &self.app {
            self.snapshot.replace(app.mailbox_list());
        }
        self.settings.account = None;
        if self.has_no_accounts() {
            self.setup.open(true);
            self.refresh_onboarding_card();
        } else {
            self.settings.refresh_in_place();
        }
    }

    /// One piece of the account pages' traffic.
    pub(super) fn accounts_input(&mut self, input: AccountsInput, sender: relm4::Sender<AppInput>) {
        match input {
            AccountsInput::Show(account) => {
                // A grant kept for another try is for the page being left.
                if account != self.settings.account {
                    self.abandon_held_nextcloud_grant();
                }
                if let Some(account) = &account {
                    // Its server is asked again which mailboxes the account's sign-in opens.
                    if let Some(app) = &self.app {
                        app.account_page_opened(account.clone());
                    }
                }
                self.settings.account = account;
                self.settings.refresh_in_place();
            }
            AccountsInput::Visit(account) => {
                self.settings
                    .record_category(super::settings::Category::Accounts);
                self.accounts_input(AccountsInput::Show(Some(account)), sender);
            }
            AccountsInput::Change(intent) => self.dispatch(Intent::AccountPage { intent }),
            AccountsInput::LinkFromSetup { account, links } => {
                self.link_from_setup(account, links, sender);
            }
            AccountsInput::SignInAgain { account, adding } => {
                self.start_account_consent(account, adding, ConsentFrom::Settings, sender);
            }
            AccountsInput::Consented(finished) => self.account_consent_finished(&finished),
            AccountsInput::Changed(Some(notice)) => self.settings.notify(notice),
            AccountsInput::Changed(None) => self.settings.refresh_in_place(),
            AccountsInput::Refused(notice) => self.settings.say(notice),
            AccountsInput::SignIn(input) => self.sign_in_input(input, sender),
            AccountsInput::Separate(input) => self.separate_input(input, sender),
            AccountsInput::AllowServerChange {
                account,
                review,
                from,
            } => {
                self.allow_server_change(account, review, from, sender);
            }
            AccountsInput::ServerChangeReplaced {
                account,
                review,
                from,
            } => Self::ask_server_change_again(&account, &review, from, &sender),
            AccountsInput::ServerChangeFailed(account) => {
                self.settings
                    .open_on_account(account, super::server_change::failed());
            }
        }
    }

    /// The core's settings signal, for the account pages: what a change came to, said once, and
    /// the list again once the account on screen is gone.
    pub(super) fn account_work_signalled(&mut self) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if let Some(account) = self.settings.account.clone() {
            if app
                .accounts_snapshot()
                .accounts
                .iter()
                .any(|entry| entry.id == account)
            {
                self.say_account_notice(app.account_page_notice(account));
            } else {
                self.account_removed();
            }
        }
        self.say_account_notice(app.accounts_notice());
    }

    fn say_account_notice(&mut self, raised: Option<AccountChangeNoticeRaised>) {
        if let Some(raised) = raised.filter(|raised| raised.id > self.settings.core_notice) {
            self.settings.core_notice = raised.id;
            self.settings
                .notify(super::settings::account_rows::change_notice(&raised.notice));
        }
    }

    /// Makes the links setup's link step chose, one after another.
    fn link_from_setup(
        &self,
        account: String,
        links: Vec<(LinkSlot, String)>,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        // The link is stored through the host's keyring, which may block.
        std::thread::spawn(move || {
            for (slot, target) in links {
                let notice = app
                    .set_account_link(account.clone(), slot, Some(target))
                    .err()
                    .map(|error| change_failed(error.to_string()));
                sender.emit(AppInput::Accounts(AccountsInput::Changed(notice)));
            }
        });
    }
}
