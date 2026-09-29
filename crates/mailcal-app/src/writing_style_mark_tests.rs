//! The keyword a reply sent from a draft carries on its Sent copy, and what learning makes of it.

use engine_api::{Draft, EmailAddress, Keyword, MessageIdHeader};

use super::{
    FakeProvider, SOURCE, account, account_id, app, fixture, found_in, install, msg, sent,
};
use crate::{LearnRange, ReplyDraftRequest, writing_style::WRITING_ASSISTANT};

fn outgoing() -> Draft {
    Draft::new(
        MessageIdHeader::new("reply@example.eu").unwrap(),
        EmailAddress::new("sam@example.eu"),
        vec![EmailAddress::new("anna@example.eu")],
        "Re: Budget",
        "Fine by me on Friday.",
    )
}

fn writing_assistant() -> Keyword {
    Keyword::new(WRITING_ASSISTANT).unwrap()
}

/// A reply sent from a draft this session issued asks for the keyword on its Sent copy, so the
/// person's other devices leave it out of learning too.
#[tokio::test]
async fn a_reply_sent_from_a_draft_asks_for_the_keyword_on_its_sent_copy() {
    let (app, _) = fixture().await;
    install(&app, mailcal_ai::Class::EuNative);
    app.learn_writing_style(
        &account_id(),
        LearnRange::default(),
        "Work".to_owned(),
        "en",
    )
    .await
    .unwrap();
    let draft = app
        .draft_reply(&ReplyDraftRequest {
            message: msg("acct-1", "in-1"),
            from: None,
            style: None,
            intent: None,
            language: None,
            ui_language: "en".to_owned(),
        })
        .await
        .unwrap();

    let marked = app.note_ai_draft_sent("acct-1", &draft.draft_id, "Fine by me.", outgoing());

    assert!(marked.sent_copy_keywords.contains(&writing_assistant()));
    assert!(app.writing_style.observed.is_assisted("reply@example.eu"));
}

/// A draft id the session never issued (one from before a restart, or made up) logs nothing and
/// leaves the message as it was.
#[tokio::test]
async fn an_unknown_draft_id_logs_nothing_and_marks_nothing() {
    let (app, _) = fixture().await;
    let unmarked = app.note_ai_draft_sent("acct-1", "never-issued", "Hello", outgoing());
    assert!(app.writing_style.observed.sends().is_empty());
    assert!(unmarked.sent_copy_keywords.is_empty());
}

/// A Sent copy carrying the keyword is left out of learning on a device whose own log never saw
/// it: the reply was drafted on another device.
#[tokio::test]
async fn learning_passes_over_a_message_marked_on_another_device() {
    let mut marked = sent("s-2026", "2026-06-01T10:00:00Z");
    marked.keywords.insert(writing_assistant());
    let provider = FakeProvider::with_sent_and_archive(vec![
        sent("s-2024", "2024-06-01T10:00:00Z"),
        sent("s-2025", "2025-06-01T10:00:00Z"),
        marked,
    ])
    .with_source(SOURCE);
    let surfaces = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let app = Box::new(app(vec![account("acct-1", provider)], &surfaces));
    app.dispatch(crate::Intent::RefreshMail).await;

    assert_eq!(found_in(&app).await, 2);
}
