//! Tests for **rich reply**: that the shared rich-draft path derives the
//! recipient/subject/threading from the stored original (exactly as the plain versions
//! did) and carries the composer-rendered HTML body and attachment manifest. A child of
//! [`super`] (the rich-submit tests), reusing its `rich_document` and `SilentObserver`
//! fixtures; split into its own file to keep each test module under the 500-line limit.
//!
//! The reply/forward composer fixture lives here and the forward tests are a child module
//! on it; the provider they all run over is [`super::mail_fake`].

use engine_api::{EmailAddress, MessageIdHeader};
use engine_core::{
    ids::{MailboxId, MessageId},
    mail::Message,
    membership::Memberships,
};
use mailcal_composer::{AttachmentId, ComposerDocument, DraftBlobHandle};

use super::{
    SilentObserver,
    mail_fake::{ThreadProvider, app_over, dispatch_until, original_message, reply_app},
    rich_document,
};
use crate::{ComposerBlob, Intent, MessageRef, SendStatus};

/// A small rich document (bold text + an inline image + a file attachment) and the bytes
/// for its blobs; shared by the reply and forward tests so they assert a real composer
/// render (a plain draft would have no HTML body and no attachments).
fn reply_document() -> (ComposerDocument, Vec<ComposerBlob>) {
    let inline_blob = DraftBlobHandle::new("blob-inline").unwrap();
    let file_blob = DraftBlobHandle::new("blob-file").unwrap();
    let document = rich_document(
        AttachmentId::new("inline-chart").unwrap(),
        AttachmentId::new("file-report").unwrap(),
        inline_blob.clone(),
        file_blob.clone(),
    );
    let blobs = vec![
        ComposerBlob::new(inline_blob, vec![1, 2, 3]),
        ComposerBlob::new(file_blob, b"PDF!".to_vec()),
    ];
    (document, blobs)
}

#[tokio::test(start_paused = true)]
async fn rich_reply_threads_and_carries_the_composer_html() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    // Sync first so the original is in the store and the reply can derive its headers.
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    // The host pre-fills the recipients from `reply_recipients` (here a plain reply: To =
    // Reply-To, no Cc) and may add a Bcc; the reply send uses exactly what it is given.
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: "discreet@remote.test".to_owned(),
        subject: None,
        document,
        blobs,
        composition: None,
        ai_draft: None,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    assert_eq!(submissions.len(), 1);
    let draft = &submissions[0];
    // Recipients are the ones supplied; Bcc carried through; Re: subject derived in core.
    assert_eq!(draft.to.len(), 1);
    assert_eq!(draft.to[0].email, "reply@remote.test");
    assert!(draft.cc.is_empty());
    assert_eq!(draft.bcc.len(), 1);
    assert_eq!(draft.bcc[0].email, "discreet@remote.test");
    assert_eq!(draft.subject, "Re: Quarterly report");
    // Rich body: the HTML alternative came from the composer render, not a plain draft.
    assert_eq!(draft.text_body, "Hello [Chart]");
    let html = draft
        .html_body
        .as_deref()
        .expect("rich reply has an HTML body");
    assert!(html.starts_with("<!DOCTYPE html><html><head>"));
    assert!(html.contains("<img src=\"cid:chart@test.local\""));
    assert_eq!(draft.attachments.len(), 2);
    // Threading: In-Reply-To the parent, References = original's chain + the parent.
    assert_eq!(
        draft.in_reply_to.as_ref().map(MessageIdHeader::as_str),
        Some("parent@remote")
    );
    let references: Vec<&str> = draft
        .references
        .iter()
        .map(MessageIdHeader::as_str)
        .collect();
    assert_eq!(references, vec!["root@remote", "parent@remote"]);
}

#[tokio::test]
async fn reply_recipients_for_a_plain_reply_is_reply_to_with_no_cc() {
    let (app, _submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, false).await;
    // To is the Reply-To (wins over From); a plain reply suggests no Cc.
    assert_eq!(suggestion.to, "reply@remote.test");
    assert_eq!(suggestion.cc, "");
}

#[tokio::test]
async fn reply_recipients_for_reply_all_keeps_to_in_to_and_cc_in_cc() {
    let (app, _submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, true).await;
    // To leads with the Reply-To, then keeps the original's other To recipients (Outlook
    // behaviour): the account's own identity (`me@allodia.local`) removed. Cc keeps the
    // original Cc; `reply@remote.test` is already in To, so it isn't repeated in Cc.
    assert_eq!(suggestion.to, "reply@remote.test, colleague@remote.test");
    assert_eq!(suggestion.cc, "boss@remote.test");
}

#[tokio::test]
async fn reply_all_keeps_a_co_recipient_in_to_not_cc() {
    // The reported case: an inbox message addressed To: me + another person, Cc: a third.
    // Reply-all must KEEP the other To recipient in To (Outlook/Thunderbird behaviour), not
    // demote them to Cc.
    let mut original = Message::new(
        MessageId::try_from("m1").unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    original.envelope.subject = Some("Team update".to_owned());
    original.envelope.from = vec![EmailAddress::new("sender@remote.test")];
    original.envelope.to = vec![
        EmailAddress::new("me@allodia.local"),
        EmailAddress::new("alice@remote.test"),
    ];
    original.envelope.cc = vec![EmailAddress::new("bob@remote.test")];

    let (app, _submissions) = reply_app(vec![original]);
    app.dispatch(Intent::RefreshMail).await;

    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, true).await;
    // To = the sender + the OTHER original To recipient (me removed); Cc = the original Cc.
    assert_eq!(suggestion.to, "sender@remote.test, alice@remote.test");
    assert_eq!(suggestion.cc, "bob@remote.test");
}

#[tokio::test]
async fn reply_recipients_is_empty_when_the_original_is_unknown() {
    let (app, _submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    // A key not in the account's synced set yields an empty suggestion rather than a guess.
    let message = MessageRef::from_parts("acct-1", "missing".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, true).await;
    assert_eq!(suggestion.to, "");
    assert_eq!(suggestion.cc, "");
}

#[tokio::test]
async fn reply_to_your_own_message_keeps_the_sender_in_to() {
    // Replying to a message in your own Sent folder: From == the account identity. The To must
    // still carry the sender (you), not be emptied by self-exclusion; otherwise the reply is
    // unsendable.
    let mut sent = Message::new(
        MessageId::try_from("m1").unwrap(),
        Memberships::of_one(MailboxId::try_from("inbox").unwrap()),
    );
    sent.envelope.subject = Some("Notes to self".to_owned());
    sent.envelope.from = vec![EmailAddress::new("me@allodia.local")];
    sent.envelope.to = vec![EmailAddress::new("me@allodia.local")];

    let (app, _submissions) = reply_app(vec![sent]);
    app.dispatch(Intent::RefreshMail).await;
    let message = MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap();
    let suggestion = app.reply_recipients(message, false).await;
    assert_eq!(suggestion.to, "me@allodia.local");
    assert_eq!(suggestion.cc, "");
}

#[tokio::test(start_paused = true)]
async fn rich_reply_with_no_recipients_fails_without_sending() {
    // The command surface guards against a recipient-less send for every caller (not just the
    // clients' Send-button gating): all of To/Cc/Bcc empty (or whitespace-only) → a Failed
    // hint, never a queued empty draft.
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    let (document, blobs) = reply_document();
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: String::new(),
        cc: "   ".to_owned(),
        bcc: String::new(),
        subject: None,
        document,
        blobs,
        composition: None,
        ai_draft: None,
    };
    let _task = dispatch_until(&app, intent, SendStatus::Failed).await;
    assert_eq!(app.send_status(), SendStatus::Failed);
    assert!(submissions.lock().unwrap().is_empty());
}

// The forward tests (threading, and the original's files travelling with it) live in their own
// file, as a child module reusing this file's fixtures.
#[path = "mail_ops_forward_tests.rs"]
mod forward;

// Submit-time quote-body hardening (re-sanitisation + `data:`→`cid:` inline re-attachment)
// lives in its own file (each test module stays under the 500-line limit), as a child module it
// reuses this module's `reply_app`/`original_message`/`dispatch_until` fixtures.
#[path = "mail_ops_reply_quote_tests.rs"]
mod quote;

// The send-from-account tests (the composer's From dropdown + the default-send-account fallback)
// live in their own file, as a child module reusing this module's two-provider fixtures.
#[path = "mail_ops_from_tests.rs"]
mod from_account;

// The editable-subject tests, likewise a child module on this file's fixtures.
#[path = "mail_ops_reply_subject_tests.rs"]
mod subject;

// The sender-name tests (what goes in `Name <address>`), a child module for the same reason,
// reusing the same fixtures plus `from_account::new_mail`.
#[path = "mail_ops_sender_name_tests.rs"]
mod sender_name;
