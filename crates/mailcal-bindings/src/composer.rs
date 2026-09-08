//! FFI helpers for the shared rich composer contract.

use std::sync::Arc;

use engine_api::AccountId;
use mailcal_app::{
    ComposerBlob as AppComposerBlob, Intent as AppIntent, MessageRef,
    RecipientSuggestion as AppRecipientSuggestion,
};
use mailcal_composer::{ComposerDocument, ComposerOutput, DraftBlobHandle};

use crate::{MailcalApp, MailcalError};

/// Host-resolved bytes for one composer blob handle.
#[derive(uniffi::Record)]
pub struct ComposerBlob {
    /// The opaque blob handle emitted by the composer output.
    pub handle: String,
    /// The attachment bytes for that handle.
    pub bytes: Vec<u8>,
}

/// The recipient fields a host's composer collects, each a comma-separated address list
/// the core splits on send. `cc`/`bcc` may be empty; `bcc` recipients are delivered but
/// hidden from every other recipient. A host pre-fills `to`/`cc` for a reply from
/// [`MailcalApp::reply_recipients`]; `bcc` is always the user's own addition.
#[derive(uniffi::Record)]
pub struct Recipients {
    /// The `To` recipients (comma-separated addresses).
    pub to: String,
    /// The `Cc` recipients (comma-separated addresses; may be empty).
    pub cc: String,
    /// The `Bcc` recipients (comma-separated addresses; may be empty); delivered but
    /// hidden from the other recipients.
    pub bcc: String,
}

/// Validates a serialized [`mailcal_composer::ComposerDocument`] and returns a
/// serialized [`mailcal_composer::ComposerOutput`].
///
/// Clients keep editor UX native-to-WebView while asking Rust for the canonical HTML,
/// text fallback, and attachment manifest. Hosts may call this before
/// [`MailcalApp::submit_rich_mail`] to validate and preview the exact output contract.
///
/// # Errors
///
/// Returns [`MailcalError::Composer`] if the JSON cannot be parsed, the document fails
/// validation, or the rendered output cannot be serialized.
#[uniffi::export]
pub fn render_composer_document_json(document_json: String) -> Result<String, MailcalError> {
    let document: mailcal_composer::ComposerDocument = serde_json::from_str(&document_json)
        .map_err(|err| MailcalError::Composer(format!("invalid composer document JSON: {err}")))?;
    let output = mailcal_composer::render(&document)
        .map_err(|err| MailcalError::Composer(err.to_string()))?;
    serde_json::to_string(&output)
        .map_err(|err| MailcalError::Composer(format!("cannot serialize composer output: {err}")))
}

/// The subject a reply to `original` opens with: `Re: <original>`, never doubled.
///
/// The composer's Subject field is editable on a reply, so what it opens with is what gets sent
/// unless the user changes it. That makes this the core's answer, not each client's: a
/// client-side `"Re: " + subject` differs from the core's on the two cases that matter (a reply to
/// a reply, and an empty subject), and the difference would now reach the wire.
///
/// The prefix is not localised: it is what threads the conversation in every other mail client.
#[must_use]
#[uniffi::export]
pub fn reply_subject(original: String) -> String {
    mailcal_app::reply_subject(Some(&original))
}

/// The subject a forward of `original` opens with: `Fwd: <original>`, on the same terms as
/// [`reply_subject`].
#[must_use]
#[uniffi::export]
pub fn forward_subject(original: String) -> String {
    mailcal_app::forward_subject(Some(&original))
}

#[uniffi::export]
impl MailcalApp {
    /// Sends a rich composer document through the durable outbox. `document_json` is a
    /// serialized [`mailcal_composer::ComposerDocument`]; `blobs` supplies the bytes for
    /// attachment handles referenced by that document. Fire-and-forget: this returns
    /// once the document validates and every required blob handle has bytes, and the
    /// observer fires when the async send/refresh completes.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Composer`] if `document_json` is invalid, document
    /// validation fails, a blob handle is blank, a required blob is missing, or a
    /// supplied byte length disagrees with known attachment metadata. Provider send
    /// failures are recorded through the outbox like existing plain-text submission and
    /// are not returned synchronously.
    ///
    /// `from` is the account id the user picked in the composer's From dropdown. Omit it (the
    /// default) to let the core derive the sending account: the selected account, else the
    /// app-level default send account, else the first configured one.
    #[uniffi::method(default(from = None))]
    pub fn submit_rich_mail(
        &self,
        recipients: Recipients,
        subject: String,
        document_json: String,
        blobs: Vec<ComposerBlob>,
        from: Option<String>,
    ) -> Result<(), MailcalError> {
        let (document, blobs) = prepare_rich(&document_json, blobs)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_dispatch(AppIntent::SubmitRichMail {
            from: send_account(from)?,
            to,
            cc,
            bcc,
            subject,
            document,
            blobs,
        });
        Ok(())
    }

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
    /// the original.
    // Eight because a reply names eight things, not because two concerns are tangled: the
    // original (account + key), the recipients, the subject, the body, its attachments, and the
    // sending account. Bundling any of them into a record would only rename the arity.
    #[allow(clippy::too_many_arguments)]
    #[uniffi::method(default(from = None, subject = None))]
    pub fn submit_rich_reply(
        &self,
        account: String,
        key: String,
        recipients: Recipients,
        document_json: String,
        blobs: Vec<ComposerBlob>,
        from: Option<String>,
        subject: Option<String>,
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
    /// Subject field; omit it to let the core derive `Fwd:` from the original.
    // Eight because a reply names eight things, not because two concerns are tangled: the
    // original (account + key), the recipients, the subject, the body, its attachments, and the
    // sending account. Bundling any of them into a record would only rename the arity.
    #[allow(clippy::too_many_arguments)]
    #[uniffi::method(default(from = None, subject = None))]
    pub fn submit_rich_forward(
        &self,
        account: String,
        key: String,
        recipients: Recipients,
        document_json: String,
        blobs: Vec<ComposerBlob>,
        from: Option<String>,
        subject: Option<String>,
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
        });
        Ok(())
    }

    /// Suggests the recipients to pre-fill a reply's editable `To`/`Cc` fields for the
    /// message `key` (in `account`): `reply_all = false` suggests just the `To` (the
    /// original's `Reply-To`, else `From`), `reply_all = true` additionally puts every other
    /// thread participant in `Cc`, minus the user's own identity. The user may edit the
    /// fields before sending; `Bcc` is never suggested (always the user's own addition).
    /// Returns empty fields when the original isn't in the account's synced set.
    ///
    /// Blocks briefly on the internal runtime (it reads the stored original), like
    /// [`MailcalApp::add_account`]; a malformed `account`/`key` yields an empty suggestion.
    #[must_use]
    pub fn reply_recipients(
        &self,
        account: String,
        key: String,
        reply_all: bool,
    ) -> crate::RecipientSuggestion {
        let Ok(message) = message_ref(&account, key) else {
            // A malformed account/key yields an empty suggestion (the composer opens blank),
            // built through the same conversion the success path uses.
            return AppRecipientSuggestion::default().into();
        };
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move { app.reply_recipients(message, reply_all).await })
            .into()
    }
}

impl MailcalApp {
    /// Schedules a fire-and-forget rich dispatch on the internal runtime: the shared
    /// back half of the rich submit/reply/forward methods, after the document has
    /// validated and every blob handle has bytes.
    fn spawn_dispatch(&self, intent: AppIntent) {
        let app = Arc::clone(&self.app);
        self.runtime.spawn(async move {
            app.dispatch(intent).await;
        });
    }
}

/// Parses `document_json` into a [`ComposerDocument`], renders it to validate, and
/// resolves every blob handle to bytes: the shared front half of the rich submit/reply/
/// forward methods, returning the document and app-typed blobs ready to dispatch.
fn prepare_rich(
    document_json: &str,
    blobs: Vec<ComposerBlob>,
) -> Result<(ComposerDocument, Vec<AppComposerBlob>), MailcalError> {
    let document: ComposerDocument = serde_json::from_str(document_json)
        .map_err(|err| MailcalError::Composer(format!("invalid composer document JSON: {err}")))?;
    let output = mailcal_composer::render(&document)
        .map_err(|err| MailcalError::Composer(err.to_string()))?;
    validate_blob_bytes(&output, &blobs)?;
    let blobs = blobs
        .into_iter()
        .map(app_blob)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((document, blobs))
}

/// Binds a row's owning-account id and provider `key` into one [`MessageRef`], mapping a
/// malformed pair to [`MailcalError::Engine`]: so a reply/forward can't carry a key
/// without (or mismatched against) its owning account. Shared by the file-attachment and
/// attachment-save FFI modules.
pub(crate) fn message_ref(account: &str, key: String) -> Result<MessageRef, MailcalError> {
    MessageRef::from_parts(account, key)
        .ok_or_else(|| MailcalError::Engine("invalid message reference".to_owned()))
}

/// Parses the host's optional From-dropdown account id into a typed [`AccountId`]. `None` (the
/// default) means "derive the sending account". A **malformed** id is rejected rather than
/// dropped to `None`: silently deriving a different sender than the user picked is the failure
/// mode this parameter exists to prevent. An id that is well-formed but names no configured
/// account can only be detected in the app (which owns the account set), where it fails the send.
/// Shared by the plain and file-attachment rich submit paths.
pub(crate) fn send_account(from: Option<String>) -> Result<Option<AccountId>, MailcalError> {
    from.map(|id| {
        AccountId::try_from(id.as_str())
            .map_err(|_| MailcalError::Engine("invalid from-account id".to_owned()))
    })
    .transpose()
}

fn app_blob(blob: ComposerBlob) -> Result<AppComposerBlob, MailcalError> {
    let handle = DraftBlobHandle::new(blob.handle)
        .ok_or_else(|| MailcalError::Composer("composer blob handle is blank".to_owned()))?;
    Ok(AppComposerBlob::new(handle, blob.bytes))
}

fn validate_blob_bytes(
    output: &ComposerOutput,
    blobs: &[ComposerBlob],
) -> Result<(), MailcalError> {
    for attachment in output
        .inline_attachments
        .iter()
        .chain(output.attachments.iter())
    {
        // An attachment with no handle carries its own bytes in the document (a pasted or
        // dropped picture), so there is no host blob to match it against.
        let Some(handle) = attachment.blob.as_ref() else {
            continue;
        };
        let Some(blob) = blobs.iter().find(|blob| blob.handle == handle.as_str()) else {
            return Err(MailcalError::Composer(format!(
                "missing bytes for composer blob {}",
                handle.as_str()
            )));
        };
        if let Some(expected) = attachment.size
            && expected != blob.bytes.len() as u64
        {
            return Err(MailcalError::Composer(format!(
                "composer blob {} has {} bytes but expected {expected}",
                handle.as_str(),
                blob.bytes.len()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ComposerBlob, Recipients, render_composer_document_json};
    use crate::{LogLevel, Logger, MailcalApp, MailcalError, Observer, Surface};

    /// A [`Logger`] that drops every record; these tests don't assert on logs.
    struct NoopLogger;

    impl Logger for NoopLogger {
        fn log(&self, _level: LogLevel, _target: String, _message: String) {}
    }

    #[test]
    fn composer_json_helper_returns_canonical_output() {
        let document = r#"{
            "blocks": [
                {
                    "Paragraph": {
                        "content": [
                            {
                                "Text": {
                                    "text": "Hello",
                                    "bold": true,
                                    "italic": false,
                                    "underline": false
                                }
                            }
                        ]
                    }
                }
            ],
            "attachments": []
        }"#;

        let output = render_composer_document_json(document.to_owned()).expect("rendered");

        assert!(output.contains(r#""html":"<!DOCTYPE html><html><head>"#));
        assert!(output.contains(r#"<body><p><strong>Hello</strong></p></body></html>"#));
        assert!(output.contains(r#""plain_text":"Hello""#));
    }

    #[test]
    fn rich_submit_rejects_a_missing_blob_before_scheduling() {
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
        let document = r#"{
            "blocks": [
                {
                    "Paragraph": {
                        "content": [
                            {
                                "Image": {
                                    "attachment_id": "inline",
                                    "alt_text": "Chart",
                                    "width_px": null
                                }
                            }
                        ]
                    }
                }
            ],
            "attachments": [
                {
                    "id": "inline",
                    "blob": "missing-blob",
                    "file_name": "chart.png",
                    "media_type": "image/png",
                    "size": 3,
                    "disposition": {
                        "Inline": {
                            "cid": "chart@test.local"
                        }
                    }
                }
            ]
        }"#;

        let err = app
            .submit_rich_mail(
                Recipients {
                    to: "you@test.local".to_owned(),
                    cc: String::new(),
                    bcc: String::new(),
                },
                "Rich".to_owned(),
                document.to_owned(),
                vec![ComposerBlob {
                    handle: "other".to_owned(),
                    bytes: vec![1, 2, 3],
                }],
                None,
            )
            .unwrap_err();

        assert!(err.to_string().contains("missing-blob"));
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
            )
            .unwrap_err();

        assert!(matches!(err, MailcalError::Engine(_)));
    }

    #[test]
    fn send_account_parses_an_id_and_rejects_a_malformed_one() {
        use super::send_account;

        // Omitted (the FFI default) means "derive the sending account".
        assert!(send_account(None).expect("no from").is_none());
        // A real row's account id parses into the typed reference the intent carries.
        let parsed = send_account(Some("acct-1".to_owned())).expect("valid from");
        assert_eq!(parsed.expect("some").as_str(), "acct-1");
        // A malformed id is an error, NOT a silent `None`; falling back to the derived account
        // would send the message as a sender the user never picked.
        assert!(matches!(
            send_account(Some(String::new())),
            Err(MailcalError::Engine(_))
        ));
    }
}
