//! What follows a connect: the link step, the accounts an "Add another account" adds on the way,
//! and the name each of them is asked for once the flow ends.
//!
//! Its own file because it spans both ways in, the password connect and the provider sign-ins,
//! and because the window it drives stays open across several of them.

use relm4::ComponentSender;

use super::{AppInput, AppModel, setup_links::LinkStep};

impl AppModel {
    /// An account was added: back to the link step it was added from, on to its own link step
    /// when another account can fill something it lacks, or the end of the flow.
    pub(super) fn after_account_added(&mut self, account: String, sender: relm4::Sender<AppInput>) {
        self.setup.added.push(account.clone());
        // A connect or sign-in still running when "Add another account" was cancelled finishes
        // onto the link step that cancel returned to, which stays on screen.
        let target = self
            .setup
            .returning_to
            .take()
            .or_else(|| self.setup.linking().map(str::to_owned))
            .unwrap_or(account);
        match self.link_step(&target) {
            Some(step) => self.setup.show_links_again(step),
            None => self.finish_setup(sender),
        }
    }

    fn link_step(&self, account: &str) -> Option<LinkStep> {
        let app = self.app.as_ref()?;
        LinkStep::for_account(&app.accounts_snapshot(), account)
    }

    /// Closes the window and asks each added account's name, one after another.
    fn finish_setup(&mut self, sender: relm4::Sender<AppInput>) {
        let added = std::mem::take(&mut self.setup.added);
        self.setup.complete();
        self.host_tasks.sender_names_waiting.extend(added);
        if self.host_tasks.sender_name_ask.is_none() {
            self.ask_next_sender_name(sender);
        }
    }

    /// Stores a name. Settings' own name field sends one too, so only the prompt's answer moves
    /// the queue on; an edit there leaves the prompt on screen where it is.
    pub(super) fn set_sender_name(
        &mut self,
        account: String,
        name: String,
        sender: relm4::Sender<AppInput>,
    ) {
        let answers_prompt = self
            .host_tasks
            .sender_name_ask
            .as_ref()
            .is_some_and(|ask| ask.account == account);
        if let Some(app) = &self.app {
            app.set_account_sender_name(account, name);
        }
        if answers_prompt {
            self.ask_next_sender_name(sender);
        }
    }

    /// The name step answered or was skipped; the next account waiting is asked, and once none
    /// is, what a sign-in's grant withheld is said on that account's page in Settings.
    pub(super) fn ask_next_sender_name(&mut self, sender: relm4::Sender<AppInput>) {
        self.host_tasks.sender_name_ask = None;
        if let Some(next) = self.host_tasks.sender_names_waiting.pop_front() {
            self.ask_sender_name(next, sender);
        } else if let Some((account, notice)) = self.host_tasks.withheld.take() {
            self.settings.open_on_account(account, notice);
        }
    }

    /// Cancel, or the window closed: back to the link step an "Add another account" left, or
    /// the end of the flow. A flow that already added an account ends as a skipped link step
    /// does, so each account added is still asked its name.
    pub(super) fn cancel_account_setup(&mut self, sender: relm4::Sender<AppInput>) {
        let Some(account) = self.setup.returning_to.take() else {
            if self.setup.added.is_empty() {
                self.setup.cancel();
            } else {
                self.finish_setup(sender);
            }
            return;
        };
        match self.link_step(&account) {
            Some(step) => self.setup.show_links_again(step),
            None => self.finish_setup(sender),
        }
    }

    /// A suggestion the core found after the link step was drawn reaches it, unless the person
    /// has changed the step already.
    pub(super) fn refresh_link_step(&mut self) {
        let Some(account) = self.setup.linking().map(str::to_owned) else {
            return;
        };
        if let Some(fresh) = self.link_step(&account) {
            self.setup.refresh_links(fresh);
        }
    }

    /// One message of the link step's.
    pub(super) fn update_setup_flow(&mut self, message: AppInput, sender: &ComponentSender<Self>) {
        let input = sender.input_sender().clone();
        match message {
            AppInput::SetupLinkPicked(picker, option) => self.setup.pick_link(picker, option),
            AppInput::SetupAddLinkedAccount => self.setup.add_linked(),
            AppInput::SetupLinksDone(link) => {
                if link && let Some(step) = &self.setup.links {
                    for (slot, target) in step.links() {
                        input.emit(AppInput::Accounts(
                            super::account_settings::AccountsInput::SetLink {
                                account: step.account.clone(),
                                slot,
                                target: Some(target),
                            },
                        ));
                    }
                }
                self.finish_setup(input);
            }
            AppInput::SenderNameNotNeeded => self.ask_next_sender_name(input),
            other => unreachable!("not a setup flow message: {other:?}"),
        }
    }
}
