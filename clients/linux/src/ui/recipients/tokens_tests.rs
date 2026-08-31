//! The recipient field's split, asserted directly; both of its failure modes are silent.

use super::{
    accept, committed, current_token, field_text, is_empty, remove, seeded, should_show_suggestions,
};

fn suggestions(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// The query is the trailing token, never the whole field. Sent whole, it matches nothing the
/// moment a first recipient is entered: which reads as "autosuggest doesn't work here".
#[test]
fn the_query_is_the_token_after_the_last_comma() {
    assert_eq!(current_token(""), "");
    assert_eq!(current_token("gr"), "gr");
    assert_eq!(current_token("ada@example.test, gr"), "gr");
    // A finished recipient leaves nothing in progress, so the core returns nothing and the list
    // closes rather than offering everyone.
    assert_eq!(current_token("ada@example.test, "), "");
}

/// Everything before the last comma is settled, and a stray separator is not a recipient.
#[test]
fn finished_recipients_are_everything_before_the_last_comma() {
    assert!(committed("gr").is_empty(), "nothing is finished yet");
    assert_eq!(committed("ada@example.test, gr"), vec!["ada@example.test"]);
    assert_eq!(
        committed("ada@example.test, bram@example.test, "),
        vec!["ada@example.test", "bram@example.test"]
    );
    assert_eq!(committed(",, ada@example.test, "), vec!["ada@example.test"]);
}

/// A field the composer was *opened* with has nothing in progress: every address a caller supplies
/// is finished. Left raw, a reply-all's last recipient sits in the input as half-typed text and a
/// single-address Cc draws no pill at all; the field looks like it dropped the people it holds.
#[test]
fn a_seeded_field_has_every_address_finished_and_stays_that_way() {
    assert_eq!(seeded(""), "", "an empty field must not become a separator");
    assert_eq!(seeded("ada@example.test"), "ada@example.test, ");
    assert_eq!(
        seeded("ada@example.test, bram@example.test"),
        "ada@example.test, bram@example.test, "
    );
    assert_eq!(committed(&seeded("ada@example.test")).len(), 1);
    assert_eq!(current_token(&seeded("ada@example.test")), "");
    // Idempotent: re-seeding an already-normalised field changes nothing.
    let once = seeded("ada@example.test, bram@example.test");
    assert_eq!(seeded(&once), once);
}

/// Accepting replaces only the token, and inserts the address bare; a display name adds nothing
/// the core uses, and one containing a comma would split into two invalid recipients.
#[test]
fn accepting_a_suggestion_keeps_the_recipients_already_entered() {
    assert_eq!(accept("gr", "greta@example.test"), "greta@example.test, ");
    assert_eq!(
        accept("ada@example.test, gr", "greta@example.test"),
        "ada@example.test, greta@example.test, "
    );
    assert_eq!(
        current_token(&accept("ada@example.test, gr", "greta@example.test")),
        "",
        "an accepted address is finished, so nothing is left in progress"
    );
}

/// Removing a pill leaves the half-typed recipient alone, and a stale index is survivable.
#[test]
fn removing_a_pill_keeps_what_is_still_being_typed() {
    let field = "ada@example.test, bram@example.test, gr";
    assert_eq!(remove(field, 0), "bram@example.test, gr");
    assert_eq!(remove(field, 1), "ada@example.test, gr");
    assert_eq!(remove(field, 9), field, "a stale index changes nothing");
    assert_eq!(remove("", 0), "");
}

/// Send is gated on this, and separators alone are not a recipient.
#[test]
fn a_field_of_separators_is_empty() {
    assert!(is_empty(""));
    assert!(is_empty(" , , "));
    assert!(!is_empty("gr"));
    assert!(!is_empty("ada@example.test, "));
}

/// A blank token returns nothing; a dropdown of everyone you have ever emailed, the moment the
/// field takes focus, is noise rather than help.
#[test]
fn the_list_stays_shut_for_a_blank_token_and_for_one_already_finished() {
    let matches = suggestions(&["greta@example.test", "gregor@example.test"]);
    assert!(!should_show_suggestions("", &matches));
    assert!(!should_show_suggestions("ada@example.test, ", &matches));
    assert!(should_show_suggestions("gr", &matches));
    assert!(!should_show_suggestions("gr", &[]));
    // Already typed in full; offering it back covers the field below for nothing.
    assert!(!should_show_suggestions("greta@example.test", &matches));
    assert!(!should_show_suggestions("GRETA@example.test", &matches));
}

/// The field is assembled in exactly one place, so a removed pill and an accepted suggestion
/// cannot drift apart on spacing.
#[test]
fn the_field_is_assembled_from_its_two_halves() {
    assert_eq!(field_text(&[], ""), "");
    assert_eq!(field_text(&[], "gr"), "gr");
    assert_eq!(
        field_text(&["ada@example.test"], "gr"),
        "ada@example.test, gr"
    );
    let field = "ada@example.test, bram@example.test, gr";
    assert_eq!(field_text(&committed(field), current_token(field)), field);
}
