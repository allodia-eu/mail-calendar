//! The section's wording: a biller or store this build cannot name still appears, a refusal is
//! its own sentence, and an amount is priced in the reader's own punctuation.

use mailcal_bindings::{AllodiaBiller, AllodiaStore, AllodiaWriteFailure};

use super::{biller_name, minor_units, store_name, write_failure_text};

/// Each refusal is its own sentence, and an outage only says nothing changed.
#[test]
fn each_refusal_is_its_own_sentence() {
    assert_ne!(
        write_failure_text(AllodiaWriteFailure::NotSwitchable),
        write_failure_text(AllodiaWriteFailure::AlreadyCancelled)
    );
    assert_ne!(
        write_failure_text(AllodiaWriteFailure::NotSwitchable),
        write_failure_text(AllodiaWriteFailure::Unexplained)
    );
}

/// A store's button names the store whose page it opens.
#[test]
fn a_store_is_named_as_people_know_it() {
    assert_eq!(store_name(AllodiaStore::Google), "Google Play");
    assert_eq!(store_name(AllodiaStore::Apple), "Apple");
}

/// Minor units and an ISO code, formatted by this side because the core carries no locale data.
/// The code rather than a symbol: this client carries no currency data, and a borrowed symbol
/// prices an amount in the wrong money while looking entirely correct.
#[test]
fn an_amount_is_priced_in_the_readers_own_punctuation_and_named_outright() {
    assert_eq!(minor_units(499, "EUR", "nl"), "4,99 EUR");
    assert_eq!(minor_units(499, "EUR", "en"), "4.99 EUR");
    // The hundredths keep both digits, or 4,90 is drawn as 4,9 and reads as a different price.
    assert_eq!(minor_units(490, "EUR", "nl"), "4,90 EUR");
    assert_eq!(minor_units(4900, "EUR", "nl"), "49,00 EUR");
    // A currency the service did not name is still the right amount.
    assert_eq!(minor_units(499, "", "nl"), "4,99");
}

/// A biller this build cannot name still has to appear, or a "you are being charged twice" warning
/// would name only one of the two.
#[test]
fn a_biller_this_build_cannot_name_is_still_named() {
    assert_eq!(biller_name(&AllodiaBiller::Allodia), "Allodia");
    assert_eq!(
        biller_name(&AllodiaBiller::Unknown {
            label: "A Shop".to_owned()
        }),
        "A Shop"
    );
}
