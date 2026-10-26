//! Every way into a composer opens through one call, and joins the composition to the
//! conversation the message belongs to when it opens: so a reply or forward saved as a draft
//! before it was ever sent carries its threading, and a stored one reopens as what it was
//! (`docs/drafts.md`).
//!
//! A child of [`super`] (the rich-submit tests), over [`super::mail_fake`]'s provider.

use engine_api::{Draft, Message, MessageIdHeader, ThreadId, ThreadRef};
use mailcal_composer::{Block, ComposerDocument, Paragraph};

use super::mail_fake::{
    ThreadProvider, app_and_logs, original_message, rich_stored_draft, stored_draft,
};
use crate::{
    ComposeKind, ComposeOrigin, ComposePrefill, CompositionId, DraftsIntent, Intent, MessageRef,
};

fn composition(name: &str) -> CompositionId {
    CompositionId::new(name).expect("a composition id")
}

fn original() -> MessageRef {
    MessageRef::from_parts("acct-1", "m1".to_owned()).expect("a message reference")
}

fn staging_dir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("mailcal-open-staging-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

fn empty_document() -> ComposerDocument {
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph::new(Vec::new()))],
        attachments: Vec::new(),
    }
}

fn save(composition: &CompositionId) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: composition.clone(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Re: Quarterly report".to_owned(),
        document: empty_document(),
        blobs: Vec::new(),
        then_close: false,
    })
}

fn chain(draft: &Draft) -> Vec<&str> {
    draft
        .references
        .iter()
        .map(MessageIdHeader::as_str)
        .collect()
}

/// The reply a user starts and leaves for later is saved as a reply. Only the send used to know
/// which message it answered, so every save before it stored a new message, and a reply resumed
/// from Drafts went out as one.
#[tokio::test]
async fn a_reply_saved_before_it_is_sent_answers_its_message() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let reply = composition("reply");
    app.open_composer(
        reply.clone(),
        ComposeOrigin::Reply {
            message: original(),
            all: false,
        },
        None,
        &staging_dir("reply"),
    )
    .await
    .expect("the reply opens");

    app.dispatch(save(&reply)).await;

    let puts = logs.draft_puts.lock().unwrap();
    let (saved, _) = puts.last().expect("the draft is saved");
    assert_eq!(
        saved.in_reply_to.as_ref().map(MessageIdHeader::as_str),
        Some("parent@remote")
    );
    assert_eq!(chain(saved), vec!["root@remote", "parent@remote"]);
}

/// A forward continues the conversation without answering anyone, saved or sent.
#[tokio::test]
async fn a_forward_saved_continues_its_conversation_without_answering() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let forward = composition("forward");
    app.open_composer(
        forward.clone(),
        ComposeOrigin::Forward {
            message: original(),
        },
        None,
        &staging_dir("forward"),
    )
    .await
    .expect("the forward opens");

    app.dispatch(save(&forward)).await;

    let puts = logs.draft_puts.lock().unwrap();
    let (saved, _) = puts.last().expect("the draft is saved");
    assert_eq!(saved.in_reply_to, None);
    assert_eq!(chain(saved), vec!["root@remote", "parent@remote"]);
}

/// A reply opens addressed, titled and quoting its original, from one call.
#[tokio::test]
async fn a_reply_opens_addressed_with_its_subject() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;

    let opening = app
        .open_composer(
            composition("reply-all"),
            ComposeOrigin::Reply {
                message: original(),
                all: true,
            },
            None,
            &staging_dir("reply-all"),
        )
        .await
        .expect("the reply opens");

    assert_eq!(opening.kind, ComposeKind::ReplyAll);
    assert_eq!(opening.account, "acct-1");
    assert_eq!(opening.to, "reply@remote.test, colleague@remote.test");
    assert_eq!(opening.cc, "boss@remote.test");
    assert_eq!(opening.subject, "Re: Quarterly report");
    assert!(opening.seeds_signature);
    assert!(opening.body_html.is_empty(), "a reply holds no stored body");
}

/// What a link, a share or an assistant puts in a composer is laid over what its origin opens
/// with, field by field.
#[tokio::test]
async fn a_prefill_is_laid_over_the_origin() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;

    let opening = app
        .open_composer(
            composition("prefilled"),
            ComposeOrigin::Reply {
                message: original(),
                all: false,
            },
            Some(ComposePrefill {
                body_text: "Yes, Thursday works.".to_owned(),
                bcc: "me@allodia.local".to_owned(),
                ..ComposePrefill::default()
            }),
            &staging_dir("prefilled"),
        )
        .await
        .expect("the reply opens");

    assert_eq!(opening.to, "reply@remote.test", "the reply's own To stays");
    assert_eq!(opening.bcc, "me@allodia.local");
    assert_eq!(opening.body_text, "Yes, Thursday works.");
    assert_eq!(opening.subject, "Re: Quarterly report");
}

/// A stored reply reopens as a reply, told apart by its own `In-Reply-To`, and carries no
/// signature to seed: its body has the one it was written with.
#[tokio::test]
async fn a_stored_reply_reopens_as_a_reply() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![rich_stored_draft("rich-1")]));
    app.dispatch(Intent::RefreshMail).await;

    let opening = app
        .open_composer(
            composition("resumed"),
            ComposeOrigin::Draft {
                message: MessageRef::from_parts("acct-1", "rich-1".to_owned()).unwrap(),
            },
            None,
            &staging_dir("resumed"),
        )
        .await
        .expect("the draft resumes");

    assert_eq!(opening.kind, ComposeKind::Reply);
    assert!(!opening.seeds_signature);
}

/// A stored message opens with its quoted original only beside its HTML, where the editor
/// completes the attribution the stored quote carries one style of. A stored text has no quote to
/// complete, and a host reads a quote beside no HTML as a fresh reply's: it would quote the
/// original a second time under text that already quotes it, and the next save would store both.
#[tokio::test]
async fn only_a_stored_message_with_html_opens_with_its_quote() {
    let threaded = |mut message: Message| {
        message.thread = Some(ThreadRef::provider_assigned(
            ThreadId::try_from("t1").expect("a thread id"),
        ));
        message
    };
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![
        threaded(original_message("m1")),
        threaded(rich_stored_draft("rich-1")),
        threaded(rich_stored_draft("plain-1")),
    ]));
    app.dispatch(Intent::RefreshMail).await;
    let resume = |key: &'static str| {
        let app = std::sync::Arc::clone(&app);
        async move {
            app.open_composer(
                composition(key),
                ComposeOrigin::Draft {
                    message: MessageRef::from_parts("acct-1", key.to_owned()).unwrap(),
                },
                None,
                &staging_dir(key),
            )
            .await
            .expect("the draft resumes")
        }
    };

    let html = resume("rich-1").await;
    assert_ne!(html.body_html, "");
    assert!(
        html.quote.is_some(),
        "a stored HTML reply opens with its original"
    );

    let text = resume("plain-1").await;
    assert_eq!(text.body_html, "");
    assert_ne!(text.body_text, "");
    assert!(
        text.quote.is_none(),
        "a stored text reply opened with a quote of its own"
    );
}

/// A reply's send counts as a reply in the feature-adoption count, however the composer was
/// opened, because the composition says so rather than the intent.
#[tokio::test]
async fn a_composition_knows_what_its_composer_is_doing() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let forward = composition("counted");
    app.open_composer(
        forward.clone(),
        ComposeOrigin::Forward {
            message: original(),
        },
        None,
        &staging_dir("counted"),
    )
    .await
    .expect("the forward opens");

    assert_eq!(app.composition_kind(Some(&forward)), ComposeKind::Forward);
    assert_eq!(app.composition_kind(None), ComposeKind::New);
}

/// An assistant's draft brings its own sign-off, so the account's signature is not seeded under
/// it; a link's or a share's body is the user's to sign (`docs/mcp.md`).
#[tokio::test]
async fn a_signed_prefill_seeds_no_signature() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;

    let open = |name: &'static str, signed: bool| {
        let app = std::sync::Arc::clone(&app);
        async move {
            app.open_composer(
                composition(name),
                ComposeOrigin::New { account: None },
                Some(ComposePrefill {
                    body_text: "Hello".to_owned(),
                    signed,
                    ..ComposePrefill::default()
                }),
                &staging_dir(name),
            )
            .await
            .expect("the composer opens")
        }
    };

    assert!(!open("assistant", true).await.seeds_signature);
    assert!(open("link", false).await.seeds_signature);
}

/// An assistant's reply to a message that cannot be found opens as a new message holding its words;
/// a reply the user started, which brings no words of its own, refuses.
#[tokio::test]
async fn a_prefilled_reply_whose_original_is_gone_opens_as_a_new_message() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;
    let gone = || ComposeOrigin::Reply {
        message: MessageRef::from_parts("acct-1", "gone".to_owned()).unwrap(),
        all: false,
    };

    let opening = app
        .open_composer(
            composition("assistant-reply"),
            gone(),
            Some(ComposePrefill {
                to: "ada@remote.test".to_owned(),
                body_text: "Thursday works.".to_owned(),
                signed: true,
                ..ComposePrefill::default()
            }),
            &staging_dir("assistant-reply"),
        )
        .await
        .expect("the assistant's words are not lost");
    assert_eq!(opening.kind, ComposeKind::New);
    assert_eq!(opening.to, "ada@remote.test");
    assert_eq!(opening.body_text, "Thursday works.");

    let refused = app
        .open_composer(
            composition("user-reply"),
            gone(),
            None,
            &staging_dir("user-reply"),
        )
        .await;
    assert!(
        refused.is_err(),
        "a reply the user started still needs its message"
    );
}

/// An assistant may answer a message in one account from another: the prefill's account is the one
/// the reply goes out from, while it stays in the original's conversation.
#[tokio::test]
async fn a_prefill_names_the_account_to_write_from() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;

    let opening = app
        .open_composer(
            composition("from-elsewhere"),
            ComposeOrigin::Reply {
                message: original(),
                all: false,
            },
            Some(ComposePrefill {
                account: Some(engine_api::AccountId::try_from("acct-2").unwrap()),
                ..ComposePrefill::default()
            }),
            &staging_dir("from-elsewhere"),
        )
        .await
        .expect("the reply opens");

    assert_eq!(opening.account, "acct-2");
    assert_eq!(opening.kind, ComposeKind::Reply);
}

/// A reply stored with an HTML body, its quote a region of it.
const HTML_REPLY: &[u8] = b"From: me@allodia.local
To: reply@remote.test
Subject: Re: Quarterly report
Message-ID: <html-reply@allodia.local>
In-Reply-To: <parent@remote>
References: <root@remote> <parent@remote>
Content-Type: text/html; charset=utf-8

<p>Thanks, looks good.</p><blockquote type=\"cite\">The report.</blockquote>
";

/// Reopens the stored reply `key`, served as `source`, beside the message it answers.
async fn reopen_reply(key: &str, source: &[u8]) -> crate::ComposeOpening {
    let mut draft = stored_draft(key);
    draft.envelope.subject = Some("Re: Quarterly report".to_owned());
    let parent = MessageIdHeader::new("parent@remote").expect("a Message-ID");
    draft.envelope.in_reply_to = vec![parent.clone()];
    draft.envelope.references = vec![
        MessageIdHeader::new("root@remote").expect("a Message-ID"),
        parent,
    ];
    let provider = ThreadProvider::with(vec![original_message("m1"), draft]).serving(key, source);
    let (app, _logs) = app_and_logs(provider);
    app.dispatch(Intent::RefreshMail).await;
    app.open_composer(
        composition(key),
        ComposeOrigin::Draft {
            message: MessageRef::from_parts("acct-1", key.to_owned()).unwrap(),
        },
        None,
        &staging_dir(key),
    )
    .await
    .expect("the draft resumes")
}

/// A reopened quote is completed from the original's headers in the store, and not its body, which
/// the draft already holds: so completing the attribution fetches nothing and works offline.
#[tokio::test]
async fn a_reopened_quote_carries_the_originals_headers_and_no_body() {
    let html = reopen_reply("html-reply", HTML_REPLY).await;
    assert_ne!(html.body_html, "");
    let quote = html
        .quote
        .expect("a reopened HTML reply completes its attribution from the original");
    assert_eq!(quote.from, "sender@remote.test");
    assert_eq!(quote.subject, "Quarterly report");
    assert!(quote.body_html.is_empty() && quote.body_plain.is_empty());
}
