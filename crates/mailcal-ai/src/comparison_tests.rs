use serde_json::{Value, json};

use super::{ComparisonExport, ModelLine, summarise};
use crate::{DraftContent, DraftExport, DraftRecord, RatedDraft, Rating, Verdict, wire::Usage};

fn draft(model: &str, variant: Option<&str>, elapsed_ms: u64, failure: Option<&str>) -> RatedDraft {
    RatedDraft {
        record: DraftRecord {
            model: model.to_owned(),
            variant: variant.map(str::to_owned),
            schema_version: u32::from(failure.is_none()),
            language: if failure.is_none() { "nl" } else { "" }.to_owned(),
            elapsed_ms,
            usage: failure.is_none().then_some(Usage {
                prompt_tokens: 1_000,
                completion_tokens: 100,
            }),
            content: failure.is_none().then(|| DraftContent {
                reply: "Hoi Marc".to_owned(),
                summary: String::new(),
                tasks: Vec::new(),
            }),
        },
        failure: failure.map(str::to_owned),
        rating: None,
    }
}

fn run() -> ComparisonExport {
    ComparisonExport {
        messages: vec![
            DraftExport {
                id: None,
                message: Some("Kun je de tekeningen sturen?".to_owned()),
                drafts: vec![
                    draft("mistral", None, 3_000, None),
                    draft("mistral", Some("terse"), 2_000, None),
                    draft("gpt-oss", None, 9_000, Some("the answer could not be read")),
                ],
            },
            DraftExport {
                id: None,
                message: Some("Past vrijdag?".to_owned()),
                drafts: vec![
                    draft("mistral", None, 5_000, None),
                    draft(
                        "mistral",
                        Some("terse"),
                        1_000,
                        Some("the endpoint answered 400"),
                    ),
                    draft("gpt-oss", None, 8_000, Some("the answer could not be read")),
                ],
            },
        ],
    }
}

#[test]
fn a_summary_counts_each_model_and_variant_in_the_order_they_first_ran() {
    let summary = summarise(run().messages.iter().flat_map(|message| &message.drafts));

    let labels: Vec<_> = summary
        .iter()
        .map(|line| (line.model.as_str(), line.variant.as_deref()))
        .collect();
    assert_eq!(
        labels,
        [
            ("mistral", None),
            ("mistral", Some("terse")),
            ("gpt-oss", None)
        ]
    );

    let default = &summary[0];
    assert_eq!(default.succeeded, 2);
    assert!(default.failed.is_empty());
    // The median of an even count is the mean of the middle two.
    assert_eq!(default.median_elapsed_ms, Some(4_000));
    assert_eq!(
        (default.prompt_tokens, default.completion_tokens),
        (2_000, 200)
    );

    let terse = &summary[1];
    assert_eq!(terse.succeeded, 1);
    assert_eq!(terse.failed["the endpoint answered 400"], 1);
    // Only the drafts that came back are timed.
    assert_eq!(terse.median_elapsed_ms, Some(2_000));

    let failing = &summary[2];
    assert_eq!(failing.succeeded, 0);
    assert_eq!(failing.failed["the answer could not be read"], 2);
    assert_eq!(failing.median_elapsed_ms, None);
    assert_eq!(failing.prompt_tokens, 0);
}

#[test]
fn the_export_holds_every_message_with_its_drafts_and_the_summary() {
    let mut run = run();
    run.messages[0].drafts[0].rating = Some(Rating::new(Verdict::Up, [], "good"));
    let exported: Value = serde_json::from_str(&run.to_json()).unwrap();

    assert_eq!(exported["version"], 1);
    assert_eq!(exported["messages"].as_array().unwrap().len(), 2);
    let first = &exported["messages"][0];
    assert_eq!(first["message"], "Kun je de tekeningen sturen?");
    assert!(first.get("id").is_none());
    assert_eq!(first["drafts"][0]["rating"]["verdict"], "up");
    assert_eq!(
        first["drafts"][2]["failure"],
        "the answer could not be read"
    );
    assert_eq!(
        exported["summary"][2],
        json!({
            "model": "gpt-oss",
            "succeeded": 0,
            "failed": { "the answer could not be read": 2 },
            "prompt_tokens": 0,
            "completion_tokens": 0,
        })
    );
    assert_eq!(exported["summary"][1]["variant"], "terse");
    assert_eq!(exported["summary"][0]["median_elapsed_ms"], 4_000);
}

#[test]
fn a_comparison_prints_no_text() {
    let printed = format!("{:?}", run());
    assert!(!printed.contains("tekeningen"));
    assert!(!printed.contains("Hoi Marc"));
}

#[test]
fn a_model_line_names_the_model_and_may_ask_for_a_reasoning_effort() {
    assert_eq!(
        ModelLine::parse("gemma-4"),
        Ok(ModelLine {
            model: "gemma-4",
            reasoning_effort: None,
        })
    );
    assert_eq!(
        ModelLine::parse("  gemma-4   reasoning=low "),
        Ok(ModelLine {
            model: "gemma-4",
            reasoning_effort: Some("low"),
        })
    );
    for line in [
        "gemma-4 reasoning=",
        "gemma-4 thinking=on",
        "gemma-4 low",
        "  ",
    ] {
        assert!(ModelLine::parse(line).is_err(), "{line}");
    }
}
