//! The writing assistant on [`MailcalApp`] (`docs/ai.md`): whether it is offered, the own
//! endpoint's form, what an `Intent::Ai` does, and the thread its work runs on.

use std::sync::Arc;

use engine_api::Provider;
use mailcal_app::ai::{AiEndpointChange, AiEndpointEdit, AiHost, AiQueued, AiWork};

use crate::{
    AiEndpointForm, AiEndpointGaps, AiEndpointIntent, AiIntent, JurisdictionClass, MailcalApp,
    ai_host::{BindingsAiHost, build_kind},
    records_ai::endpoint_form,
};

impl MailcalApp {
    /// Gives the assistant its host and the key the keystore held at launch. Called once, as the
    /// app is built.
    pub(crate) fn install_ai(&self, key: Option<String>) {
        self.app.restore_ai_endpoint_key(key);
        let host = BindingsAiHost::new(
            self.runtime.handle().clone(),
            Arc::clone(&self.credential_store),
        );
        self.app
            .install_ai_host(Arc::new(host) as Arc<dyn AiHost>, build_kind());
    }

    /// Queues a piece of the assistant's work, and runs it on a thread of its own when it starts
    /// now. Returns at once.
    pub(crate) fn queue_ai_work(&self, work: AiWork<Box<dyn Provider>>) -> AiQueued {
        self.start_ai_work(self.app.queue_ai_work(work))
    }

    /// Runs the work just queued on a thread of its own when `queued` says it starts now: every
    /// piece of the assistant's work starts here. Returns `queued`.
    pub(crate) fn start_ai_work(&self, queued: AiQueued) -> AiQueued {
        if queued == AiQueued::Run {
            let app = Arc::clone(&self.app);
            std::thread::spawn(move || app.run_ai_work());
        }
        queued
    }

    /// Handles an `Intent::Ai`. Returns at once: a change runs on a thread of its own.
    pub(crate) fn dispatch_ai(&self, intent: AiIntent) {
        match intent {
            AiIntent::Endpoint { intent } => self.dispatch_ai_endpoint(intent),
            AiIntent::WritingStyle { intent } => self.dispatch_writing_style(intent),
            AiIntent::ReplyDraft { intent } => self.dispatch_reply_draft(intent),
        }
    }

    fn dispatch_ai_endpoint(&self, intent: AiEndpointIntent) {
        let change = match intent {
            AiEndpointIntent::Save {
                address,
                model,
                declared,
                key,
            } => AiEndpointChange::Save(AiEndpointEdit {
                address,
                model,
                declared: declared.map(Into::into),
                key,
            }),
            AiEndpointIntent::Remove => AiEndpointChange::Remove,
        };
        if self.app.queue_ai_endpoint_change(change) {
            let app = Arc::clone(&self.app);
            std::thread::spawn(move || app.run_ai_endpoint_changes());
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Whether the writing assistant is offered: Settings → Writing style, Draft a reply and the
    /// own endpoint's form are drawn exactly when this says so, and each of their records reads it
    /// too (`docs/ai.md` rule 4). Read from memory, so a UI thread may ask it.
    #[must_use]
    pub fn ai_offer(&self) -> bool {
        self.app.ai_offer()
    }

    /// The own endpoint's form, for an Advanced page to embed, or `None` while the assistant is
    /// not offered (pulled after `Surface::AiEndpoint` and `Surface::Settings`). Read from memory.
    #[must_use]
    pub fn ai_endpoint_form(&self) -> Option<AiEndpointForm> {
        endpoint_form(self.app.ai_endpoint())
    }

    /// What a save of these fields would refuse, each field's at once: what the form says beside
    /// its fields as the person types, and what the save itself refuses by.
    #[must_use]
    pub fn ai_endpoint_gap(
        &self,
        address: String,
        model: String,
        declared: Option<JurisdictionClass>,
    ) -> AiEndpointGaps {
        self.app
            .ai_endpoint_gap(&address, &model, declared.map(Into::into))
            .into()
    }
}

#[cfg(test)]
#[path = "app_ai_tests.rs"]
mod tests;
