//! Feedback on a draft, end to end through the app: a draft issued from the canned endpoint, rated,
//! and kept in the outbox with what made it, and with what it said only when asked.

use mailcal_ai::{Class, Rating, Reason, Verdict};
use serde_json::Value;

use super::{account_id, fixture, install, msg};
use crate::{LearnRange, ReplyDraftRequest};

async fn drafted(app: &crate::App<super::FakeProvider>) -> String {
    install(app, Class::EuNative);
    app.learn_writing_style(
        &account_id(),
        LearnRange::default(),
        "Work".to_owned(),
        "en",
    )
    .await
    .unwrap();
    app.draft_reply(&ReplyDraftRequest {
        message: msg("acct-1", "in-1"),
        from: None,
        style: None,
        intent: Some("secret intent".to_owned()),
        language: None,
        ui_language: "en".to_owned(),
    })
    .await
    .unwrap()
    .draft_id
}

fn body(app: &crate::App<super::FakeProvider>, index: usize) -> Value {
    serde_json::from_str(&app.ai_feedback_waiting()[index].body).unwrap()
}

#[tokio::test]
async fn feedback_carries_what_made_the_draft_and_what_it_said_only_when_asked() {
    let (app, _) = fixture().await;
    let draft = drafted(&app).await;

    let down = Rating::new(Verdict::Down, [Reason::WrongLength], "too formal");
    assert!(app.rate_draft(&draft, down.clone(), false));
    assert!(app.rate_draft(&draft, down, true));

    let waiting = app.ai_feedback_waiting();
    assert_eq!(waiting.len(), 2);
    assert!(!waiting[0].with_content && waiting[1].with_content);

    let plain = body(&app, 0);
    assert_eq!(plain["id"], waiting[0].id.as_str());
    assert!(plain.get("message").is_none());
    let record = &plain["drafts"][0];
    assert_eq!(record["model"], "mistral-small");
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["language"], "en");
    assert!(record.get("content").is_none());
    assert_eq!(record["rating"]["reasons"][0], "wrong_length");
    assert_eq!(record["rating"]["comment"], "too formal");
    assert!(!waiting[0].body.contains("figures"));

    let full = body(&app, 1);
    assert!(
        full["message"]
            .as_str()
            .unwrap()
            .starts_with("Thanks for sending the figures")
    );
    let content = &full["drafts"][0]["content"];
    assert!(content["reply"].as_str().unwrap().starts_with("Hi Anna"));
    assert_eq!(content["summary"], "Anna asks whether Friday suits.");
    assert_eq!(content["tasks"][0]["kind"], "fill_in");
    // Neither the headers, the intent nor the style travel, whatever the box says.
    for item in &waiting {
        assert!(!item.body.contains("\"from\""));
        assert!(!item.body.contains("secret intent"));
        assert!(!item.body.contains("Direct, first names."));
    }
}

#[tokio::test]
async fn a_draft_the_session_did_not_issue_is_not_rated() {
    let (app, _) = fixture().await;
    assert!(!app.rate_draft("unknown", Rating::new(Verdict::Up, [], ""), true));
    assert!(app.ai_feedback_waiting().is_empty());
}

#[tokio::test]
async fn delivered_feedback_leaves_the_outbox_and_a_sign_out_empties_it() {
    let (app, _) = fixture().await;
    let draft = drafted(&app).await;
    for _ in 0..3 {
        assert!(app.rate_draft(&draft, Rating::new(Verdict::Up, [], ""), false));
    }
    let first = app.ai_feedback_waiting()[0].id.clone();
    app.ai_feedback_delivered(&first);
    let waiting = app.ai_feedback_waiting();
    assert_eq!(waiting.len(), 2);
    assert!(waiting.iter().all(|item| item.id != first));

    app.forget_ai_feedback();
    assert!(app.ai_feedback_waiting().is_empty());
}
