//! The pure rules a composer opens by: which kind it is, which seed the editor gets, the quote's
//! attribution, and what counts as already written. No window: every rule here is silent when
//! wrong, so each is pinned on its own.

use mailcal_bindings::{
    ComposeKind, ComposeOpening, ComposerFileAttachment, QuoteSource, QuoteStyleKind,
};
use serde_json::Value;

use super::{ComposeContext, attribution_script, body_seed_script};
use crate::{
    l10n,
    ui::{composer_quote::quote_style_of, reader::ComposerHost, timestamps::MailClock},
};

const ZONE: &str = "Europe/Amsterdam";

fn opening(kind: ComposeKind) -> ComposeOpening {
    ComposeOpening {
        composition: "composer-7".to_owned(),
        kind,
        account: "work".to_owned(),
        to: "ada@example.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Re: Planning".to_owned(),
        body_html: String::new(),
        body_text: String::new(),
        quote: None,
        attachments: Vec::new(),
        attachments_failed: false,
        seeds_signature: true,
    }
}

fn original() -> QuoteSource {
    QuoteSource {
        from: "Sender <sender@example.test>".to_owned(),
        date: "2026-08-31T05:01:00Z".to_owned(),
        to: "recipient@example.test".to_owned(),
        cc: String::new(),
        subject: "Planning".to_owned(),
        body_html: "<p>Already sanitised</p>".to_owned(),
        body_plain: "Already sanitised".to_owned(),
    }
}

fn file(name: &str) -> ComposerFileAttachment {
    ComposerFileAttachment {
        path: format!("/staged/{name}"),
        file_name: name.to_owned(),
        media_type: "application/pdf".to_owned(),
    }
}

fn context(opening: ComposeOpening) -> ComposeContext {
    ComposeContext::from_opening(
        opening,
        ComposerHost::Pane,
        &QuoteStyleKind::Indented,
        &MailClock::new(ZONE, true),
    )
}

/// The argument of the one editor call `script` makes, as the editor will parse it.
fn argument(script: &str, call: &str) -> Value {
    let inner = script
        .strip_prefix(&format!("window.{call}("))
        .and_then(|rest| rest.strip_suffix(");"))
        .unwrap_or_else(|| panic!("not a {call} call: {script}"));
    let value: Value = serde_json::from_str(inner).expect("valid JSON");
    match value {
        Value::String(encoded) => serde_json::from_str(&encoded).expect("valid nested JSON"),
        other => other,
    }
}

#[test]
fn the_composer_takes_its_kind_from_the_core_a_stored_reply_included() {
    for kind in [
        ComposeKind::New,
        ComposeKind::Reply,
        ComposeKind::ReplyAll,
        ComposeKind::Forward,
    ] {
        assert_eq!(context(opening(kind)).kind, kind);
    }
    let stored = context(ComposeOpening {
        body_html: "<p>Agreed</p>".to_owned(),
        seeds_signature: false,
        ..opening(ComposeKind::Reply)
    });
    assert_eq!(stored.kind, ComposeKind::Reply);
    assert!(
        !stored.seeds_signature,
        "a stored message already carries its signature"
    );
}

#[test]
fn the_composer_opens_on_the_composition_and_account_the_core_answered_with() {
    let opened = context(opening(ComposeKind::New));
    assert_eq!(opened.composition, "composer-7");
    assert_eq!(opened.initial_from.as_deref(), Some("work"));
    let unnamed = context(ComposeOpening {
        account: String::new(),
        ..opening(ComposeKind::New)
    });
    assert_eq!(unnamed.initial_from, None);
}

/// A forward's originals and a stored draft's own files are still in the mailbox, so a composer
/// closed over them unchanged has lost nothing.
#[test]
fn the_files_the_core_staged_are_the_baseline_for_every_kind() {
    for kind in [ComposeKind::Forward, ComposeKind::New, ComposeKind::Reply] {
        let opened = context(ComposeOpening {
            attachments: vec![file("plan.pdf"), file("notes.pdf")],
            ..opening(kind)
        });
        assert_eq!(opened.files.len(), 2);
        assert_eq!(opened.opening_files, 2, "{kind:?}");
        assert_eq!(opened.files[0].path, "/staged/plan.pdf");
    }
    let failed = context(ComposeOpening {
        attachments_failed: true,
        ..opening(ComposeKind::Forward)
    });
    assert!(failed.attachments_failed);
}

#[test]
fn a_stored_message_seeds_its_own_html_and_completes_its_quote_attribution() {
    let stored = context(ComposeOpening {
        body_html: "<p><strong>Agreed</strong></p>".to_owned(),
        body_text: "Agreed".to_owned(),
        quote: Some(original()),
        ..opening(ComposeKind::Reply)
    });
    assert_eq!(
        body_seed_script(&stored).as_deref(),
        Some(
            "window.setComposerBody({\"html\":\"<p><strong>Agreed</strong></p>\",\"text\":\
             \"Agreed\"});"
        ),
        "a reopened message is seeded with its HTML, never with a second quote"
    );
    assert_eq!(stored.quote, None);
    let completion = argument(
        &attribution_script(&stored).expect("a reopened quote is completed"),
        "completeComposerQuoteAttribution",
    );
    assert_eq!(
        completion["line"],
        l10n::quote_attribution("2026-08-31 07:01", "Sender <sender@example.test>")
    );
    assert_eq!(completion["headers"][0]["label"], l10n::quote_from());

    let without_original = context(ComposeOpening {
        body_html: "<p>Agreed</p>".to_owned(),
        ..opening(ComposeKind::Reply)
    });
    assert_eq!(attribution_script(&without_original), None);
}

#[test]
fn a_fresh_reply_seeds_the_quote_with_the_prefilled_text_above_it() {
    let reply = context(ComposeOpening {
        body_text: "Sounds good".to_owned(),
        quote: Some(original()),
        ..opening(ComposeKind::Reply)
    });
    let seed = argument(
        &body_seed_script(&reply).expect("a reply is seeded"),
        "setComposerQuote",
    );
    assert_eq!(seed["style"], "Indented");
    assert_eq!(seed["body_html"], "<p>Already sanitised</p>");
    assert_eq!(seed["body_plain"], "Already sanitised");
    assert_eq!(seed["initial_text"], "Sounds good");
    assert_eq!(
        attribution_script(&reply),
        None,
        "a fresh quote carries both attributions already"
    );

    // An empty body is not a pre-filled one, so the key is left off entirely.
    let empty = context(ComposeOpening {
        quote: Some(original()),
        ..opening(ComposeKind::Reply)
    });
    let seed = argument(
        &body_seed_script(&empty).expect("a reply is seeded"),
        "setComposerQuote",
    );
    assert!(seed.get("initial_text").is_none());
}

#[test]
fn the_quoted_date_is_localised_before_it_is_sent_to_anyone() {
    // The attribution and the `Sent:` header are read by the *recipient*, so a raw UTC instant
    // from the core is a defect in their mailbox rather than in ours.
    let reply = context(ComposeOpening {
        quote: Some(QuoteSource {
            cc: "copy@example.test".to_owned(),
            ..original()
        }),
        ..opening(ComposeKind::Reply)
    });
    let seed = argument(
        &body_seed_script(&reply).expect("a reply is seeded"),
        "setComposerQuote",
    );
    let line = seed["attribution"]["line"].as_str().expect("a line");
    let headers = &seed["attribution"]["headers"];
    // Amsterdam is UTC+2 on 31 August, so the hour proves the instant was converted.
    assert_eq!(
        line,
        l10n::quote_attribution("2026-08-31 07:01", "Sender <sender@example.test>")
    );
    assert_eq!(headers[1]["label"], l10n::quote_sent());
    assert_eq!(headers[1]["value"], "2026-08-31 07:01");
    assert_eq!(headers[2]["value"], "recipient@example.test");
    assert_eq!(headers[3]["label"], l10n::quote_cc());
    assert_eq!(headers[4]["label"], l10n::quote_subject());
    assert_eq!(headers[4]["value"], "Planning");
}

#[test]
fn a_forward_is_marked_forwarded_and_lists_only_the_recipients_it_had() {
    let forward = context(ComposeOpening {
        quote: Some(QuoteSource {
            to: String::new(),
            ..original()
        }),
        ..opening(ComposeKind::Forward)
    });
    let seed = argument(
        &body_seed_script(&forward).expect("a forward is seeded"),
        "setComposerQuote",
    );
    assert_eq!(seed["attribution"]["line"], l10n::quote_forwarded());
    let labels = seed["attribution"]["headers"]
        .as_array()
        .expect("headers")
        .iter()
        .map(|header| header["label"].as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        [
            l10n::quote_from(),
            l10n::quote_sent(),
            l10n::quote_subject()
        ]
    );
}

#[test]
fn an_original_with_no_body_is_not_quoted() {
    let reply = context(ComposeOpening {
        quote: Some(QuoteSource {
            body_html: String::new(),
            body_plain: String::new(),
            ..original()
        }),
        ..opening(ComposeKind::Reply)
    });
    assert_eq!(reply.quote, None);
    assert_eq!(body_seed_script(&reply), None);
}

#[test]
fn prefilled_text_is_handed_over_as_data_never_as_code() {
    let link = context(ComposeOpening {
        body_text: "Hello </script>\nSecond line".to_owned(),
        ..opening(ComposeKind::New)
    });
    assert_eq!(
        body_seed_script(&link).as_deref(),
        Some("window.setPlainText(\"Hello </script>\\nSecond line\");")
    );
    assert_eq!(body_seed_script(&context(opening(ComposeKind::New))), None);
}

#[test]
fn a_reopened_composers_style_picker_reads_the_style_its_quote_was_saved_in() {
    let document = concat!(
        r#"{"blocks":[{"Paragraph":{"content":[]}},{"Quote":{"style":"LineAndHeader","#,
        r#""attribution":{"line":"","headers":[]},"body_html":"","body_plain":""}}],"#,
        r#""attachments":[]}"#
    );
    assert!(matches!(
        quote_style_of(document),
        Some(QuoteStyleKind::LineAndHeader)
    ));
    assert!(matches!(
        quote_style_of(&document.replace("LineAndHeader", "Indented")),
        Some(QuoteStyleKind::Indented)
    ));
    assert!(quote_style_of(r#"{"blocks":[],"attachments":[]}"#).is_none());
    assert!(quote_style_of("not json").is_none());
}

#[test]
fn only_a_message_carrying_a_quote_offers_the_style_picker() {
    assert!(
        context(ComposeOpening {
            quote: Some(original()),
            ..opening(ComposeKind::Reply)
        })
        .has_quote
    );
    assert!(
        context(ComposeOpening {
            body_html: "<p>Agreed</p>".to_owned(),
            quote: Some(original()),
            ..opening(ComposeKind::Reply)
        })
        .has_quote,
        "a reopened reply whose original is still here can switch its quote's style"
    );
    assert!(!context(opening(ComposeKind::New)).has_quote);
}
