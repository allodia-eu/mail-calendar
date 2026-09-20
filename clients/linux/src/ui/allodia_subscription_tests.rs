//! What an answered write is allowed to put on screen.
//!
//! Every arm here is a sentence about somebody's money, and three of them are silent when wrong: a
//! cancellation with no date reads as "it stops now", a switch reported without the service's own
//! figures quotes a price the subscriber will not be charged, and a checkout that only opened a
//! browser has nothing to report yet and must not claim otherwise.

use super::{SubscriptionAnswer, SubscriptionState, WriteOutcome, note_for};
use crate::ui::allodia_subscription_facts::WriteFailure;

/// ⚠️ The date is the whole of it: somebody cancelling wants to know they are not losing what they
/// have already paid for.
#[test]
fn a_cancellation_names_the_day_access_runs_to() {
    let note = note_for(
        &WriteOutcome::Cancelled("2026-11-01T00:00:00+00:00".to_owned()),
        "nl",
        "EUR",
    )
    .expect("a cancellation always has something to say");
    assert!(note.contains("1 nov 2026"), "{note}");
}

/// A date this build cannot read still lands in the sentence, because one with no date at all is
/// not true.
#[test]
fn a_cancellation_with_an_unreadable_date_keeps_the_wire() {
    let note = note_for(&WriteOutcome::Cancelled("whenever".to_owned()), "en", "EUR")
        .expect("a cancellation always has something to say");
    assert!(note.contains("whenever"), "{note}");
}

/// ⚠️ **The service's own figures, never today's list price.** A price change never reaches
/// somebody who already subscribed, so quoting the list price would tell a long-standing
/// subscriber a number they will not be charged.
#[test]
fn a_switch_reports_the_amount_the_service_named() {
    let note = note_for(
        &WriteOutcome::Switched {
            next_payment_date: Some("2026-11-01".to_owned()),
            amount_in_cents: 4900,
        },
        "nl",
        "EUR",
    )
    .expect("a switch always has something to say");
    assert!(note.contains("49,00 EUR"), "{note}");
    assert!(note.contains("1 nov 2026"), "{note}");
}

/// The browser has the rest of it, and the re-read is what will report the outcome. A sentence
/// here would be a claim about something that has not happened yet.
#[test]
fn a_page_opened_in_the_browser_claims_nothing() {
    assert!(
        note_for(
            &WriteOutcome::SentToBrowser("https://example.test/pay".to_owned()),
            "en",
            "EUR"
        )
        .is_none()
    );
}

/// A restart that needed no payment page says so, and a refusal says which refusal it was.
#[test]
fn every_other_ending_says_what_it_was() {
    assert!(note_for(&WriteOutcome::Restarted, "en", "EUR").is_some());
    let refused = note_for(
        &WriteOutcome::Failed(WriteFailure::AlreadyCancelled),
        "en",
        "EUR",
    )
    .expect("a refusal always has something to say");
    let unexplained = note_for(
        &WriteOutcome::Failed(WriteFailure::Unexplained),
        "en",
        "EUR",
    )
    .expect("a failure always has something to say");
    assert_ne!(
        refused, unexplained,
        "a refusal and a service that could not be reached are different things to say"
    );
}

/// ⚠️ **Coming back to the window is the only signal a checkout gives**, because the payment
/// finishes in a browser and Settings stays open behind it. What it must not become is a round
/// trip every time this window regains focus on any of its eleven pages, so a return is worth
/// asking about only where an answer is already drawn.
#[test]
fn a_return_asks_again_only_where_an_answer_is_already_drawn() {
    let mut state = SubscriptionState::default();
    assert!(
        !state.worth_rereading(),
        "nothing has been drawn, so there is nothing to bring up to date"
    );

    state.answer = Some(SubscriptionAnswer::Unavailable);
    assert!(
        state.worth_rereading(),
        "a section that answered at all is one somebody is looking at"
    );

    // A read already running will answer for the return anyway, and two at once would race each
    // other's answers onto the same card.
    state.checking = true;
    assert!(!state.worth_rereading());
}
