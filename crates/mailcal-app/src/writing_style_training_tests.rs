//! Comparing drafts in a debug build: the answered Inbox messages a run can take, a model named
//! per request and instructions named per variant, both reaching the endpoint through the gate,
//! and a refusal recorded as a failure.

use std::sync::{Arc, Mutex};

use engine_api::{EmailAddress, MessageIdHeader};
use mailcal_ai::{AiError, Class, GatedBackend, OwnEndpoint};
use serde_json::Value;

use super::{
    FakeEndpoint, FakeProvider, SOURCE, account, account_id, app, at, fixture, install, message,
    msg, sent,
};
use crate::{App, Intent, LearnRange, ReplyDraftRequest, WritingStyleError};

/// Two Inbox messages, the older answered by a message in Sent that names it in `In-Reply-To` and
/// the newer not answered at all.
async fn answered_fixture() -> Box<App<FakeProvider>> {
    let id = |value: &str| vec![MessageIdHeader::new(value).unwrap()];
    let mut answered = message("in-1", "a", "Budget");
    answered.envelope.from = vec![EmailAddress::named("Anna", "anna@example.eu")];
    answered.envelope.message_id = id("in-1@example.eu");
    answered.received_at = Some(at("2026-07-01T09:00:00Z"));
    let mut unanswered = message("in-2", "a", "Lunch");
    unanswered.envelope.message_id = id("in-2@example.eu");
    unanswered.received_at = Some(at("2026-07-03T09:00:00Z"));
    let mut reply = sent("s-1", "2026-07-02T10:00:00Z");
    reply.envelope.in_reply_to = id("in-1@example.eu");
    let provider =
        FakeProvider::with_sent_and_archive(vec![answered, unanswered, reply]).with_source(SOURCE);
    let app = Box::new(app(vec![account("acct-1", provider)], &Arc::default()));
    app.dispatch(Intent::RefreshMail).await;
    app
}

#[tokio::test]
async fn the_inbox_messages_an_account_with_a_style_answered_are_offered_newest_first() {
    let app = answered_fixture().await;
    // An account that drafts in no style has nothing to compare.
    assert!(app.training_answered(5).await.is_empty());

    install(&app, Class::EuNative);
    app.learn_writing_style(
        &account_id(),
        LearnRange::default(),
        "Work".to_owned(),
        "en",
    )
    .await
    .unwrap();
    let answered = app.training_answered(5).await;

    assert_eq!(answered.len(), 1);
    assert_eq!(answered[0].account, account_id());
    assert_eq!(answered[0].mail.key.as_str(), "in-1");
    assert_eq!(answered[0].mail.from_name.as_deref(), Some("Anna"));
    assert_eq!(answered[0].mail.subject.as_deref(), Some("Budget"));
    assert!(app.training_answered(0).await.is_empty());
}

fn backend(
    app: &crate::App<super::FakeProvider>,
    model: &str,
    declared: Class,
) -> (GatedBackend, Arc<Mutex<Vec<Value>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let backend = GatedBackend::own_endpoint(
        OwnEndpoint::new("http://localhost:11434/v1", None, model, Some(declared)).unwrap(),
        Box::new(FakeEndpoint {
            seen: Arc::clone(&seen),
        }),
        app.jurisdiction_mode_source(),
    );
    (backend, seen)
}

fn request() -> ReplyDraftRequest {
    ReplyDraftRequest {
        message: msg("acct-1", "in-1"),
        from: None,
        style: None,
        intent: None,
        language: None,
        ui_language: "en".to_owned(),
    }
}

#[tokio::test]
async fn each_model_and_variant_reaches_the_endpoint_and_is_recorded() {
    let (app, _) = fixture().await;
    install(&app, Class::EuNative);
    app.learn_writing_style(
        &account_id(),
        LearnRange::default(),
        "Work".to_owned(),
        "en",
    )
    .await
    .unwrap();

    let (llama, seen) = backend(&app, "llama-3", Class::EuNative);
    let (record, failure) = app
        .training_draft(
            &request(),
            &llama,
            Some(("terse", "Answer in {reply_language}, briefly.\n\n{closing}")),
        )
        .await;

    assert!(failure.is_none());
    assert_eq!(record.model, "llama-3");
    assert_eq!(record.variant.as_deref(), Some("terse"));
    assert_eq!(record.schema_version, 1);
    let content = record.content.unwrap();
    assert!(content.reply.starts_with("Hi Anna"));
    let sent = seen.lock().unwrap()[0].clone();
    assert_eq!(sent["model"], "llama-3");
    assert!(
        sent["messages"][0]["content"]
            .as_str()
            .unwrap()
            .starts_with("Answer in English, briefly.")
    );

    let message = app.training_message(&msg("acct-1", "in-1")).await.unwrap();
    assert!(message.starts_with("Thanks for sending the figures"));
}

#[tokio::test]
async fn a_refused_model_is_recorded_as_a_failure_and_nothing_is_sent() {
    let (app, _) = fixture().await;
    install(&app, Class::EuNative);
    app.learn_writing_style(
        &account_id(),
        LearnRange::default(),
        "Work".to_owned(),
        "en",
    )
    .await
    .unwrap();

    let (abroad, seen) = backend(&app, "gpt", Class::NonEu);
    let (record, failure) = app.training_draft(&request(), &abroad, None).await;

    assert!(matches!(
        failure,
        Some(WritingStyleError::Ai(AiError::Refused(_)))
    ));
    assert_eq!(record.model, "gpt");
    assert!(record.content.is_none());
    assert!(seen.lock().unwrap().is_empty());
}
