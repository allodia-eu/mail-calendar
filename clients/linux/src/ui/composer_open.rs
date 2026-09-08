//! Everything fixed at the moment a composer opens: who it is addressed to, what it quotes, what
//! it is called, and which account it will go out as.
//!
//! Split out of [`super`] so the shell file stays the Relm4 component; this is the one place the
//! four compose entry points (new, reply, reply-all, forward) agree on a request.

use super::{
    AppModel,
    composer_model::{ComposeKind, ComposeRequest, initial_sender, quote_seed},
};

impl AppModel {
    pub(super) fn begin_compose(&mut self, kind: ComposeKind) {
        let Some(app) = &self.app else {
            return;
        };
        let opened = self.reading.opened.as_ref();
        if kind != ComposeKind::New && opened.is_none() {
            return;
        }
        let (initial_to, initial_cc) = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                let recipients = app.reply_recipients(
                    message.account.clone(),
                    message.key.clone(),
                    kind == ComposeKind::ReplyAll,
                );
                (recipients.to, recipients.cc)
            }
            _ => (String::new(), String::new()),
        };
        let quote = opened.and_then(|message| {
            let settings = app.quote_settings();
            // Only a showcase reply arrives pre-written, so the store screenshot shows a written
            // reply rather than an empty body. Every real reply passes `None`.
            #[cfg(any(debug_assertions, feature = "dev-harness"))]
            let initial_text = (kind == ComposeKind::Reply || kind == ComposeKind::ReplyAll)
                .then(|| crate::showcase::reply_text(&message.account, &message.key))
                .flatten();
            #[cfg(not(any(debug_assertions, feature = "dev-harness")))]
            let initial_text: Option<String> = None;
            quote_seed(
                message,
                &self.reading.snapshot,
                &settings.style,
                kind == ComposeKind::Forward,
                initial_text.as_deref(),
                self.calendar.display_zone(),
            )
        });
        // Derived by the CORE, not here. The field is editable, so what it opens with is what gets
        // sent unless the user changes it, and a client-side "Re: " + subject differs from the
        // core's on a reply to a reply.
        let subject = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                mailcal_bindings::reply_subject(message.subject.clone())
            }
            (ComposeKind::Forward, Some(message)) => {
                mailcal_bindings::forward_subject(message.subject.clone())
            }
            _ => String::new(),
        };
        self.composer_generation = self.composer_generation.wrapping_add(1);
        self.composer_error = false;
        self.composer = Some(ComposeRequest {
            kind,
            account: opened.map(|message| message.account.clone()),
            key: opened.map(|message| message.key.clone()),
            initial_to,
            initial_cc,
            initial_bcc: String::new(),
            subject,
            initial_body: None,
            quote,
            initial_from: initial_sender(
                opened,
                self.snapshot.selected_account.as_deref(),
                app.default_send_account(),
            ),
            seeds_signature: true,
        });
    }
}
