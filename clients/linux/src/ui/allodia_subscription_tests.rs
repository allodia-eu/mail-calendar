//! What an answered write puts on screen. Every arm is a sentence about somebody's money, and two
//! are silent when wrong: a cancellation with no date reads as "it stops now", and a switch
//! reported without the service's own figures quotes a price the subscriber will not be charged.

use mailcal_bindings::{AllodiaSubscriptionOutcome, AllodiaSubscriptionState, AllodiaWriteFailure};

use super::{outcome_text, worth_rereading};

/// ⚠️ The date is the whole of it: somebody cancelling wants to know they are not losing what they
/// have already paid for.
#[test]
fn a_cancellation_names_the_day_access_runs_to() {
    let said = outcome_text(
        &AllodiaSubscriptionOutcome::Cancelled {
            end_date: "2026-11-01T00:00:00+00:00".to_owned(),
        },
        "nl",
    );
    assert!(said.contains("1 nov 2026"), "{said}");
}

/// A date this build cannot read still lands in the sentence.
#[test]
fn a_cancellation_with_an_unreadable_date_keeps_the_wire() {
    let said = outcome_text(
        &AllodiaSubscriptionOutcome::Cancelled {
            end_date: "whenever".to_owned(),
        },
        "en",
    );
    assert!(said.contains("whenever"), "{said}");
}

/// ⚠️ **The service's own figures, in the currency the account is billed in.**
#[test]
fn a_switch_reports_the_amount_the_service_named() {
    let said = outcome_text(
        &AllodiaSubscriptionOutcome::Switched {
            next_payment_date: Some("2026-11-01".to_owned()),
            amount_in_cents: 4900,
            currency: "EUR".to_owned(),
        },
        "nl",
    );
    assert!(said.contains("49,00 EUR"), "{said}");
    assert!(said.contains("1 nov 2026"), "{said}");
}

/// A refusal says which refusal it was, and an outage only that nothing changed.
#[test]
fn a_refusal_and_an_outage_are_different_sentences() {
    let failed = |failure| outcome_text(&AllodiaSubscriptionOutcome::Failed { failure }, "en");
    assert_ne!(
        failed(AllodiaWriteFailure::AlreadyCancelled),
        failed(AllodiaWriteFailure::Unexplained)
    );
    assert_ne!(
        outcome_text(&AllodiaSubscriptionOutcome::Restarted, "en"),
        ""
    );
}

/// ⚠️ **Coming back to the window is the only signal a checkout gives**, but not a round trip
/// every time this window regains focus on any of its pages: only where an answer is drawn.
#[test]
fn a_return_asks_again_only_where_an_answer_is_drawn() {
    assert!(!worth_rereading(&AllodiaSubscriptionState::Hidden));
    assert!(!worth_rereading(&AllodiaSubscriptionState::Checking));
    assert!(worth_rereading(&AllodiaSubscriptionState::Unavailable));
    assert!(worth_rereading(&AllodiaSubscriptionState::NeedsReauth));
}
