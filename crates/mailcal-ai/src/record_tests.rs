use serde_json::{Value, json};

use super::{
    COMMENT_CHARS, DraftContent, DraftExport, DraftRecord, RatedDraft, Rating, Reason, Verdict,
};
use crate::{DraftTask, TaskKind, wire::Usage};

fn record(content: bool) -> DraftRecord {
    DraftRecord {
        model: "mistral-small".to_owned(),
        variant: None,
        schema_version: 1,
        language: "nl".to_owned(),
        elapsed_ms: 4_200,
        usage: Some(Usage {
            prompt_tokens: 1_000,
            completion_tokens: 120,
        }),
        content: content.then(|| DraftContent {
            reply: "Hoi Anna, secret reply [dag]".to_owned(),
            summary: "Anna asks to move Friday.".to_owned(),
            tasks: vec![
                DraftTask::new(TaskKind::FillIn, "[dag]"),
                DraftTask::new(TaskKind::Attach, "Attach the agenda"),
            ],
        }),
    }
}

fn exported(document: &DraftExport) -> Value {
    serde_json::from_str(&document.to_json()).unwrap()
}

#[test]
fn a_rating_keeps_each_reason_once_and_a_bounded_comment() {
    let rating = Rating::new(
        Verdict::Down,
        [Reason::WrongTone, Reason::MadeThingsUp, Reason::WrongTone],
        &format!("  {}  ", "x".repeat(COMMENT_CHARS + 50)),
    );
    assert_eq!(rating.reasons, [Reason::WrongTone, Reason::MadeThingsUp]);
    assert_eq!(rating.comment.chars().count(), COMMENT_CHARS);

    let up = Rating::new(Verdict::Up, [], "   ");
    assert!(up.comment.is_empty());
    assert_eq!(
        serde_json::to_value(&up).unwrap(),
        json!({ "verdict": "up", "reasons": [] })
    );
}

#[test]
fn feedback_without_consent_carries_what_made_the_draft_and_nothing_it_said() {
    let document = DraftExport {
        id: Some("fb-1".to_owned()),
        message: None,
        drafts: vec![RatedDraft {
            record: record(true).without_content(),
            failure: None,
            rating: Some(Rating::new(Verdict::Down, [Reason::WrongLength], "")),
        }],
    };
    assert_eq!(
        exported(&document),
        json!({
            "version": 1,
            "id": "fb-1",
            "drafts": [{
                "model": "mistral-small",
                "schema_version": 1,
                "language": "nl",
                "elapsed_ms": 4200,
                "usage": { "prompt_tokens": 1000, "completion_tokens": 120 },
                "rating": { "verdict": "down", "reasons": ["wrong_length"] },
            }],
        })
    );
    assert!(!document.to_json().contains("secret"));
}

#[test]
fn a_run_carries_the_message_every_draft_and_every_failure() {
    let mut named = record(true);
    named.variant = Some("terse".to_owned());
    let failed = DraftRecord {
        model: "llama".to_owned(),
        variant: None,
        schema_version: 1,
        language: String::new(),
        elapsed_ms: 30,
        usage: None,
        content: None,
    };
    let document = DraftExport {
        id: None,
        message: Some("Kunnen we vrijdag verplaatsen?".to_owned()),
        drafts: vec![
            RatedDraft {
                record: named,
                failure: None,
                rating: Some(Rating::new(Verdict::Up, [], "")),
            },
            RatedDraft {
                record: failed,
                failure: Some("unreachable".to_owned()),
                rating: None,
            },
        ],
    };
    let value = exported(&document);
    assert_eq!(value["message"], "Kunnen we vrijdag verplaatsen?");
    assert_eq!(value["drafts"][0]["variant"], "terse");
    assert_eq!(
        value["drafts"][0]["content"]["reply"],
        "Hoi Anna, secret reply [dag]"
    );
    assert_eq!(
        value["drafts"][0]["content"]["tasks"],
        json!([
            { "kind": "fill_in", "text": "[dag]" },
            { "kind": "attach", "text": "Attach the agenda" },
        ])
    );
    assert_eq!(value["drafts"][1]["failure"], "unreachable");
    assert!(value["drafts"][1].get("rating").is_none());
    assert!(value["drafts"][1].get("language").is_none());
}

#[test]
fn nothing_here_prints_what_a_draft_or_a_comment_said() {
    let rating = Rating::new(Verdict::Down, [Reason::SomethingElse], "secret comment");
    let printed = format!("{:?} {rating:?}", record(true));
    assert!(!printed.contains("secret"));
}
