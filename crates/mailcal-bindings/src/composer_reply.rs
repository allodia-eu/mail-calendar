//! Answering or passing on a message the user already has: the rich reply and forward
//! submits.
//!
//! Its own file rather than an addition to [`crate::composer`], which is at the 500-line
//! limit. Both take the original by `account` + `key` and share every helper there; the
//! `#[uniffi::export] impl` block below continues that one.

use mailcal_app::Intent as AppIntent;

use crate::{
    MailcalApp, MailcalError,
    composer::{ComposerBlob, Recipients, message_ref, prepare_rich, send_account},
    drafts::sent_composition,
};

#[uniffi::export]
impl MailcalApp {
    /// Replies to the message `key` (in `account`, the row's owning account) with a rich
    /// composer document. The host supplies `recipients` (its `to`/`cc` pre-filled from
    /// [`MailcalApp::reply_recipients`] for reply or reply-all, and editable by the user) and
    /// the `subject` its editable Subject field holds; the core derives the threading headers
    /// from the original.
    /// `document_json`/`blobs` carry the rich body exactly as
    /// [`MailcalApp::submit_rich_mail`]. Fire-and-forget once the document validates.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Composer`] for an invalid document or blob (as
    /// [`MailcalApp::submit_rich_mail`]), or [`MailcalError::Engine`] if the `account`/`key`
    /// reference (or a supplied `from`) is malformed.
    ///
    /// `from` is the account id the user picked in the composer's From dropdown, letting a reply
    /// go out from a different mailbox than the one that received the original. Omit it (the
    /// default) to reply from `account`. `subject` is what the user left in the composer's
    /// Subject field, which is editable on a reply; omit it to let the core derive `Re:` from
    /// the original. `composition` is as on [`MailcalApp::submit_rich_mail`].
    // Nine because a reply names nine things, not because two concerns are tangled: the
    // original (account + key), the recipients, the subject, the body, its attachments, the
    // sending account and the composer it was written in. Bundling any of them into a record
    // would only rename the arity.
    #[allow(clippy::too_many_arguments)]
    #[uniffi::method(default(from = None, subject = None, composition = None))]
    pub fn submit_rich_reply(
        &self,
        account: String,
        key: String,
        recipients: Recipients,
        document_json: String,
        blobs: Vec<ComposerBlob>,
        from: Option<String>,
        subject: Option<String>,
        composition: Option<String>,
    ) -> Result<(), MailcalError> {
        let message = message_ref(&account, key)?;
        let (document, blobs) = prepare_rich(&document_json, blobs)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_dispatch(AppIntent::SubmitRichReply {
            message,
            from: send_account(from)?,
            to,
            cc,
            bcc,
            subject,
            document,
            blobs,
            composition: sent_composition(composition)?,
        });
        Ok(())
    }

    /// Forwards the message `key` (in `account`, the row's owning account) to the
    /// host-supplied `recipients`, under the `subject` its editable Subject field holds (no
    /// threading). `document_json`/`blobs` carry the rich body exactly as
    /// [`MailcalApp::submit_rich_mail`]. Fire-and-forget once the document validates.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Composer`] for an invalid document or blob (as
    /// [`MailcalApp::submit_rich_mail`]), or [`MailcalError::Engine`] if the `account`/`key`
    /// reference (or a supplied `from`) is malformed.
    ///
    /// `from` is the account id the user picked in the composer's From dropdown. Omit it (the
    /// default) to forward from `account`. `subject` is what the user left in the composer's
    /// Subject field; omit it to let the core derive `Fwd:` from the original. `composition`
    /// is as on [`MailcalApp::submit_rich_mail`].
    // Nine because a reply names nine things, not because two concerns are tangled: the
    // original (account + key), the recipients, the subject, the body, its attachments, the
    // sending account and the composer it was written in. Bundling any of them into a record
    // would only rename the arity.
    #[allow(clippy::too_many_arguments)]
    #[uniffi::method(default(from = None, subject = None, composition = None))]
    pub fn submit_rich_forward(
        &self,
        account: String,
        key: String,
        recipients: Recipients,
        document_json: String,
        blobs: Vec<ComposerBlob>,
        from: Option<String>,
        subject: Option<String>,
        composition: Option<String>,
    ) -> Result<(), MailcalError> {
        let message = message_ref(&account, key)?;
        let (document, blobs) = prepare_rich(&document_json, blobs)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_dispatch(AppIntent::SubmitRichForward {
            message,
            from: send_account(from)?,
            to,
            cc,
            bcc,
            subject,
            document,
            blobs,
            composition: sent_composition(composition)?,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{LogLevel, Logger, MailcalApp, MailcalError, Observer, Recipients, Surface};

    /// A [`Logger`] that drops every record; this test doesn't assert on logs.
    struct NoopLogger;

    impl Logger for NoopLogger {
        fn log(&self, _level: LogLevel, _target: String, _message: String) {}
    }

    #[test]
    fn rich_reply_rejects_a_blank_account_before_scheduling() {
        struct NoopObserver;

        impl Observer for NoopObserver {
            fn surface_changed(&self, _surface: Surface) {}
        }

        let app = MailcalApp::new_demo(
            Box::new(NoopObserver),
            Box::new(NoopLogger),
            LogLevel::Info,
            "Etc/UTC".to_owned(),
        );
        // A blank account id can't be a real row's account; reply rejects it (Engine error)
        // rather than risk routing the send to the wrong account.
        let err = app
            .submit_rich_reply(
                String::new(),
                "m1".to_owned(),
                Recipients {
                    to: "someone@test.local".to_owned(),
                    cc: String::new(),
                    bcc: String::new(),
                },
                r#"{"blocks": [], "attachments": []}"#.to_owned(),
                Vec::new(),
                None,
                None,
                None,
            )
            .unwrap_err();

        assert!(matches!(err, MailcalError::Engine(_)));
    }
}
