//! What Writing style says, pinned without a window.

use mailcal_bindings::{
    AiRoute, CreditBalance, GateRefusal, HabitRow, JurisdictionClass, JurisdictionMode,
    LanguageStyleRow, OwnEndpointError, WritingStyleFailure, WritingStyleRow, WritingStyleSnapshot,
};

use super::{
    WritingStyleFeed, changed_sender, draft_script, endpoint_error_text, failure_text,
    format_credits, intent_of, languages_text, lead_is_empty, reveal_fields, shown_credits,
    style_summary,
};
use crate::{l10n, ui::timestamps};

fn snapshot(route: Option<AiRoute>, balance: Option<CreditBalance>) -> WritingStyleSnapshot {
    WritingStyleSnapshot {
        route,
        refused: None,
        styles: Vec::new(),
        accounts: Vec::new(),
        learning: None,
        balance,
    }
}

fn balance() -> CreditBalance {
    CreditBalance {
        credits: 12.5,
        as_of: 1_782_000_000,
    }
}

/// Every variant has words of its own, and the two that depend on something else say the right
/// thing for it.
#[test]
fn every_failure_is_worded_by_its_variant() {
    let refused = |mode| WritingStyleFailure::Refused {
        mode,
        class: JurisdictionClass::NonEu,
    };
    let cases = [
        (
            WritingStyleFailure::Unavailable,
            l10n::ai_error_unavailable(),
        ),
        (WritingStyleFailure::Busy, l10n::ai_error_busy()),
        (
            WritingStyleFailure::NoSentFolder,
            l10n::ai_error_no_sent_folder(),
        ),
        (
            WritingStyleFailure::NothingToLearn,
            l10n::learn_report_nothing(),
        ),
        (WritingStyleFailure::NoStyle, l10n::ai_error_no_style()),
        (WritingStyleFailure::NotFound, l10n::ai_error_not_found()),
        (
            refused(JurisdictionMode::EuNative),
            l10n::writing_style_refused_eu_native(),
        ),
        (
            refused(JurisdictionMode::EuHosted),
            l10n::writing_style_refused_eu_hosted(),
        ),
        (
            WritingStyleFailure::OutOfCredits,
            l10n::ai_error_out_of_credits(),
        ),
        (
            WritingStyleFailure::NotEntitled,
            l10n::ai_error_not_entitled(),
        ),
        (
            WritingStyleFailure::RateLimited,
            l10n::ai_error_rate_limited(),
        ),
        (
            WritingStyleFailure::Unreachable,
            l10n::ai_error_unreachable(),
        ),
        (WritingStyleFailure::Malformed, l10n::ai_error_malformed()),
        (WritingStyleFailure::Cancelled, l10n::ai_error_cancelled()),
    ];
    for (failure, expected) in cases {
        assert_eq!(
            failure_text(&failure, Some(AiRoute::Relay)),
            expected,
            "{failure:?}"
        );
    }
    assert_eq!(
        failure_text(&WritingStyleFailure::Status { code: 503 }, None),
        l10n::ai_error_status("503")
    );
}

/// A refused sign-in through the relay is fixed by signing in again; a refused own endpoint by
/// its key. Telling someone with their own server to sign in to Allodia sends them nowhere.
#[test]
fn a_refused_credential_is_the_routes_own() {
    let refused = WritingStyleFailure::Unauthorized;
    assert_eq!(
        failure_text(&refused, Some(AiRoute::Relay)),
        l10n::ai_error_sign_in_again()
    );
    assert_eq!(
        failure_text(&refused, Some(AiRoute::OwnEndpoint)),
        l10n::ai_error_key_refused()
    );
    assert_eq!(failure_text(&refused, None), l10n::ai_error_key_refused());
}

#[test]
fn each_endpoint_error_names_the_field_that_is_wrong() {
    assert_eq!(
        endpoint_error_text(&OwnEndpointError::InvalidUrl),
        l10n::ai_endpoint_error_address()
    );
    assert_eq!(
        endpoint_error_text(&OwnEndpointError::NotHttps),
        l10n::ai_endpoint_error_https()
    );
    assert_eq!(
        endpoint_error_text(&OwnEndpointError::NoModel),
        l10n::ai_endpoint_error_model()
    );
    // The keystore's own sentence is never shown: it is the platform's, not ours.
    assert_eq!(
        endpoint_error_text(&OwnEndpointError::Keystore("org.freedesktop".to_owned())),
        l10n::ai_endpoint_error_keystore()
    );
}

#[test]
fn credits_have_at_most_one_decimal_in_the_readers_punctuation() {
    assert_eq!(format_credits(12.0, "en"), "12");
    assert_eq!(format_credits(12.34, "en"), "12.3");
    assert_eq!(format_credits(0.5, "nl"), "0,5");
    assert_eq!(format_credits(1234.56, "de"), "1234,6");
    // Nothing the relay could have meant reads as a negative balance.
    assert_eq!(format_credits(-3.0, "en"), "0");
    assert_eq!(format_credits(f64::NAN, "en"), "0");
}

/// The line states a balance only while requests go through the relay that reported it: after a
/// switch to an own endpoint, the relay's last number describes nothing the person is using.
#[test]
fn a_balance_is_shown_only_on_the_relay() {
    assert!(shown_credits(&snapshot(Some(AiRoute::Relay), Some(balance()))).is_some());
    assert!(shown_credits(&snapshot(Some(AiRoute::OwnEndpoint), Some(balance()))).is_none());
    assert!(shown_credits(&snapshot(None, Some(balance()))).is_none());
    assert!(shown_credits(&snapshot(Some(AiRoute::Relay), None)).is_none());
}

#[test]
fn languages_are_named_in_their_own_language() {
    let named = languages_text(&["nl".to_owned(), "de".to_owned()]);
    assert!(named.contains("Nederlands"), "{named}");
    assert!(named.contains("Deutsch"), "{named}");
}

fn style(messages: u32, learned_at: i64, languages: &[&str]) -> WritingStyleRow {
    WritingStyleRow {
        id: "style".to_owned(),
        name: "My style".to_owned(),
        source_account: String::new(),
        languages: languages.iter().map(|code| (*code).to_owned()).collect(),
        messages,
        oldest: None,
        newest: None,
        learned_at,
    }
}

#[test]
fn a_rows_summary_says_what_it_was_learned_from_and_in_which_languages() {
    let learned_at = 1_782_000_000;
    let day = timestamps::local_day(learned_at, "UTC", "en").expect("a day");
    let summary = style_summary(&style(40, learned_at, &["en"]), "UTC", "en");
    let lines = summary.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], l10n::writing_style_learned_from(40, &day));
    assert_eq!(
        lines[1],
        l10n::writing_style_languages(&languages_text(&["en".to_owned()]))
    );

    // A style from another device carries no date, and then says only what it covers.
    let undated = style_summary(&style(40, 0, &["en"]), "UTC", "en");
    assert_eq!(undated.lines().count(), 1);
    assert!(style_summary(&style(0, 0, &[]), "UTC", "en").is_empty());
}

fn language() -> LanguageStyleRow {
    LanguageStyleRow {
        language: "en".to_owned(),
        greetings: vec![
            HabitRow {
                text: "Hi Anna".to_owned(),
                share: 60,
            },
            HabitRow {
                text: "  ".to_owned(),
                share: 10,
            },
        ],
        sign_offs: Vec::new(),
        signs_as: "Ada".to_owned(),
        register: String::new(),
        typical_words: 120,
        shape: "Short paragraphs.".to_owned(),
        punctuation: "  ".to_owned(),
        structure: String::new(),
        moves: String::new(),
        phrases: vec!["Sounds good".to_owned(), String::new()],
        avoid: Vec::new(),
    }
}

/// An empty field is skipped rather than drawn as a heading over nothing, in the reveal's order.
#[test]
fn the_reveal_lists_what_was_noticed_and_skips_what_was_not() {
    let fields = reveal_fields(&language(), "en");
    let labels = fields
        .iter()
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    let length = l10n::reveal_length(120);
    assert_eq!(
        labels,
        [
            l10n::reveal_greetings(),
            l10n::reveal_signs_as(),
            length.as_str(),
            l10n::reveal_shape(),
            l10n::reveal_phrases(),
        ]
    );
    assert_eq!(fields[0].1, "Hi Anna (60%)");
    assert_eq!(fields[1].1, "Ada");
    assert!(fields[2].1.is_empty(), "the length is in its label");
    assert_eq!(fields[4].1, "Sounds good");
}

#[test]
fn a_share_is_written_the_way_its_language_writes_a_percentage() {
    assert_eq!(reveal_fields(&language(), "de")[0].1, "Hi Anna (60\u{a0}%)");
    assert_eq!(reveal_fields(&language(), "nl")[0].1, "Hi Anna (60%)");
}

/// The core is told about a From account only when the person moved to one.
#[test]
fn only_a_changed_sender_is_passed_on() {
    assert_eq!(changed_sender(Some("work"), Some("work")), None);
    assert_eq!(
        changed_sender(Some("home"), Some("work")),
        Some("home".to_owned())
    );
    assert_eq!(changed_sender(None, Some("work")), None);
}

#[test]
fn an_intent_of_only_space_is_no_intent() {
    assert_eq!(intent_of("   "), None);
    assert_eq!(intent_of(" Yes, Tuesday "), Some("Yes, Tuesday".to_owned()));
}

/// Only a definite "nothing written" lets a draft in without asking.
#[test]
fn a_draft_asks_first_unless_the_editor_says_the_lead_is_empty() {
    assert!(lead_is_empty(Some("false")));
    assert!(!lead_is_empty(Some("true")));
    assert!(!lead_is_empty(Some("undefined")));
    assert!(!lead_is_empty(None));
}

/// A draft is data: quotes, a closing tag and a line break reach the editor exactly as written.
#[test]
fn a_draft_reaches_the_editor_as_two_string_literals() {
    let text = "He said \"yes\"</script>\nThanks,\u{2028}Ada";
    let script = draft_script(text, "draft-1");
    let arguments = script
        .strip_prefix("window.setComposerDraftText(")
        .and_then(|rest| rest.strip_suffix(");"))
        .expect("one call");
    let parsed: Vec<String> =
        serde_json::from_str(&format!("[{arguments}]")).expect("two JSON strings");
    assert_eq!(parsed, [text.to_owned(), "draft-1".to_owned()]);
}

/// The category comes and goes with AI's availability, and only that needs a rebuild; every
/// other snapshot redraws in place.
#[test]
fn only_a_route_appearing_or_going_asks_for_a_rebuild() {
    let mut feed = WritingStyleFeed::default();
    assert!(
        !feed.receive(snapshot(None, None)),
        "nothing was there before"
    );
    assert!(feed.receive(snapshot(Some(AiRoute::OwnEndpoint), None)));
    assert!(!feed.receive(snapshot(Some(AiRoute::Relay), Some(balance()))));
    let mut refused = snapshot(Some(AiRoute::Relay), None);
    refused.refused = Some(GateRefusal {
        mode: JurisdictionMode::EuNative,
        class: JurisdictionClass::NonEu,
    });
    assert!(!feed.receive(refused));
    assert!(feed.receive(snapshot(None, None)));
    assert_eq!(feed.generation(), 5, "every snapshot is a redraw");
    assert!(feed.snapshot().is_some_and(|last| last.route.is_none()));
}
