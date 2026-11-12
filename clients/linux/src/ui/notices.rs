//! The core's notices (`docs/notices.md`): pulled after `Surface::Notices` and once at launch,
//! drawn by the window's one banner, each account's row in the folder pane and the account's page
//! in Settings, and each action run through a flow this client already has, or by the core for a
//! Retry. Which notice is drawn where is [`super::notice_rules`].

use std::collections::HashMap;

use mailcal_bindings::{Intent, Notice, NoticeAction, NoticeIntent};

use super::{
    AppInput, AppModel,
    notice_rules::{self, AccountMark},
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

/// Every notice in force, in the core's order, and what is drawn from it.
#[derive(Debug, Default)]
pub(super) struct NoticesState {
    pub(super) notices: Vec<Notice>,
    pub(super) marks: HashMap<String, AccountMark>,
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
        self.notices.marks = notice_rules::marks(&notices);
        self.notices.notices = notices;
        // An account's page in Settings carries the account's notices at its top.
        self.settings.signalled();
    }

    pub(super) fn notice_input(&mut self, input: NoticeInput, sender: &relm4::Sender<AppInput>) {
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
                    self.run_notice(&notice, action, sender);
                }
            }
            NoticeInput::Second(_) => {
                if let Some(action) = notice.second_action {
                    self.run_notice(&notice, action, sender);
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
        notice: &Notice,
        action: NoticeAction,
        sender: &relm4::Sender<AppInput>,
    ) {
        if let Some(intent) = notice_rules::core_remedy(notice, action) {
            self.dispatch(Intent::Notices { intent });
            return;
        }
        // An answer already being carried out holds both; a press that raced the redraw is
        // dropped rather than sent twice.
        if notice.running {
            return;
        }
        if let Some(intent) = notice_rules::answer(notice, action) {
            // The action only: a reply's subject holds the meeting's title, which is message
            // content (`docs/logging.md`).
            log::info!("notices: a question was answered with {action:?}");
            self.dispatch(intent);
            return;
        }
        match (action, notice.account.clone()) {
            (NoticeAction::SignInAgain, Some(account)) => {
                self.sign_in_in_browser(account, Vec::new());
            }
            (NoticeAction::SignInToAllodia, _) => self.start_allodia(false),
            (NoticeAction::OpenAccountSettings, Some(account)) => {
                self.settings.open_account(account);
            }
            (NoticeAction::OpenAccountSettings, None) => {
                self.settings.open(Some(settings::Category::Accounts));
            }
            (NoticeAction::Review, Some(account)) => {
                self.review_server_change(account, sender);
            }
            _ => {}
        }
    }
}
