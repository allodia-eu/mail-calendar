//! Comparing drafts in a debug build: a model named per request and instructions named per
//! variant, both reaching the endpoint through the gate, and a refusal recorded as a failure.

use std::sync::{Arc, Mutex};

use mailcal_ai::{AiError, Class, GatedBackend, OwnEndpoint};
use serde_json::Value;

use super::{FakeEndpoint, account_id, fixture, install, msg};
use crate::{LearnRange, ReplyDraftRequest, WritingStyleError};

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
