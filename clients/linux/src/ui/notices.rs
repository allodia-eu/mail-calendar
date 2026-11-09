//! The core's notices (`docs/notices.md`): pulled after `Surface::Notices` and once at launch,
//! drawn by the window's one banner, each account's row in the folder pane and the account's page
//! in Settings, and each action run through a flow this client already has, or by the core for a
//! Retry. Which notice is drawn where is [`super::notice_rules`].

use std::collections::HashMap;

use mailcal_bindings::{Intent, Notice, NoticeAction, NoticeIntent};

use super::{
    AppInput, AppModel,
    account_consent::ConsentFrom,
    account_sign_in::Switch,
    notice_rules::{self, AccountMark, SignInFlow},
    settings,
};

/// What a notice's button asks of the model.
#[derive(Debug, Clone)]
pub(crate) enum NoticeInput {
    /// Run the action of the notice with this id.
    Run(String),
    /// Run the other answer of the question with this id (rule 8).
    Second(String),
    /// Close the notice with this id, which the core offered to close.
    Dismiss(String),
}

/// A sign-in this client runs for a notice, by the flow that runs it, so the flow's end is
/// matched to the notice that started it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Remedy {
    Consent,
    AccountSignIn,
    Jmap,
    Allodia,
}

/// Every notice in force, in the core's order, and what is drawn from it.
#[derive(Debug, Default)]
pub(super) struct NoticesState {
    pub(super) notices: Vec<Notice>,
    pub(super) marks: HashMap<String, AccountMark>,
    /// The notice whose sign-in is running, and the flow running it: its button shows a spinner
    /// until the flow ends (`docs/waiting.md` rule 1).
    running: Option<(Remedy, String)>,
}

impl NoticesState {
    /// The id of the notice whose action is running, if one is.
    pub(super) fn running(&self) -> Option<&str> {
        self.running.as_ref().map(|(_, id)| id.as_str())
    }
}

impl AppModel {
    /// Reads the notices again. A signal that changed nothing changes nothing here either, so it
    /// reaches no redraw beyond the comparison.
    pub(super) fn pull_notices(&mut self) {
        let Some(app) = &self.app else {
            return;
        };
        let notices = app.notices().notices;
        if notices == self.notices.notices {
            return;
        }
        log::info!("notices: {} in force", notices.len());
        let allodia_moved = notice_rules::allodia_sign_in_ended(&notices)
            != notice_rules::allodia_sign_in_ended(&self.notices.notices);
        self.notices.marks = notice_rules::marks(&notices);
        self.notices.notices = notices;
        // An account's page in Settings carries the account's notices at its top.
        self.settings.signalled();
        // The Allodia card says whether its sign-in ended, on a page the signal above leaves be.
        if allodia_moved {
            self.refresh_settings_in_place();
        }
    }

    pub(super) fn notice_input(&mut self, input: NoticeInput, sender: relm4::Sender<AppInput>) {
        let id = match &input {
            NoticeInput::Run(id) | NoticeInput::Second(id) | NoticeInput::Dismiss(id) => id,
        };
        // Read from what is in force now: a button drawn a snapshot ago may name one that went.
        let Some(notice) = self
            .notices
            .notices
            .iter()
            .find(|notice| &notice.id == id)
            .cloned()
        else {
            return;
        };
        match input {
            NoticeInput::Run(_) => {
                if let Some(action) = notice.action {
                    self.run_notice(notice, action, sender);
                }
            }
            NoticeInput::Second(_) => {
                if let Some(action) = notice.second_action {
                    self.run_notice(notice, action, sender);
                }
            }
            NoticeInput::Dismiss(id) if notice.dismissible => {
                self.dispatch(Intent::Notices {
                    intent: NoticeIntent::Dismiss { id },
                });
            }
            NoticeInput::Dismiss(_) => {}
        }
    }

    /// Runs `action`, one of `notice`'s answers: a Retry through the core, which judges it, a
    /// question's answer as the intent it stands for, and every other through the flow this client
    /// already has (rules 7 and 8).
    fn run_notice(
        &mut self,
        notice: Notice,
        action: NoticeAction,
        sender: relm4::Sender<AppInput>,
    ) {
        if let Some(intent) = notice_rules::core_remedy(&notice, action) {
            self.dispatch(Intent::Notices { intent });
            return;
        }
        // An answer already being carried out holds both; a press that raced the redraw is
        // dropped rather than sent twice.
        if notice.running {
            return;
        }
        if let Some(intent) = notice_rules::answer(&notice, action) {
            // The action only: a reply's subject holds the meeting's title, which is message
            // content (`docs/logging.md`).
            log::info!("notices: a question was answered with {action:?}");
            self.dispatch(intent);
            return;
        }
        match (action, notice.account.clone()) {
            (NoticeAction::SignInAgain, Some(account)) => {
                self.sign_in_again(&notice, account, sender);
            }
            // A hop already outstanding answers for it.
            (NoticeAction::SignInToAllodia, _) if !self.settings.allodia_signing_in => {
                self.notices.running = Some((Remedy::Allodia, notice.id));
                if !self.resolve_allodia_sign_in_ended(sender) {
                    self.notice_remedy_ended(Remedy::Allodia, true);
                }
            }
            (NoticeAction::OpenAccountSettings, Some(account)) => {
                self.settings.open_account(account);
            }
            (NoticeAction::OpenAccountSettings, None) => {
                self.settings.open(Some(settings::Category::Accounts));
            }
            (NoticeAction::Review, Some(account)) => {
                self.review_server_change(account, &sender);
            }
            _ => {}
        }
    }

    /// The account's sign-in again, in place: it keeps its id, mail and settings.
    fn sign_in_again(&mut self, notice: &Notice, account: String, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let nextcloud = app
            .accounts_snapshot()
            .accounts
            .iter()
            .any(|entry| entry.id == account && entry.nextcloud);
        let flow = notice_rules::sign_in_flow(
            &notice.kind,
            app.account_provider(account.clone()).as_ref(),
            nextcloud,
        );
        let remedy = match flow {
            SignInFlow::Consent => {
                self.start_account_consent(account, Vec::new(), ConsentFrom::Notice, sender);
                Remedy::Consent
            }
            SignInFlow::Jmap => {
                self.start_jmap_reauth(account, sender);
                Remedy::Jmap
            }
            SignInFlow::AccountSignIn => {
                self.start_account_sign_in(account, Switch::Again, ConsentFrom::Notice, sender);
                Remedy::AccountSignIn
            }
            SignInFlow::Settings => {
                self.settings.open_account(account);
                return;
            }
        };
        self.notices.running = Some((remedy, notice.id.clone()));
    }

    /// A sign-in `remedy` ran for a notice has ended. One that was refused is said on the notice
    /// (rule 10); a browser the person closed or left is not a failure, and its flow ends it as a
    /// cancelled one.
    pub(super) fn notice_remedy_ended(&mut self, remedy: Remedy, failed: bool) {
        let Some((running, id)) = self.notices.running.take() else {
            return;
        };
        if running != remedy {
            self.notices.running = Some((running, id));
            return;
        }
        if failed {
            log::warn!("notices: the sign-in a notice started did not complete");
            self.dispatch(Intent::Notices {
                intent: NoticeIntent::RemedyFailed { id },
            });
        }
    }
}
