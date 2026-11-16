//! How the subscription section words what the core decided
//! (`MailcalApp::allodia_subscription_card`): who is charging, a refusal's sentence, and an amount
//! in minor units. Plain data in and out, so each is decided in a unit test.

use mailcal_bindings::{AllodiaBiller, AllodiaStore, AllodiaWriteFailure};

use crate::l10n;

/// What a biller is called on screen.
///
/// **Allodia, never the payment processor behind it.** Naming a processor somebody has never heard
/// of, inside a message about being charged twice, is how a correct warning reads as a scam. The
/// bare company name is right here, where the sentence is about who is taking the money rather
/// than about the app.
pub(crate) fn biller_name(biller: &AllodiaBiller) -> String {
    match biller {
        AllodiaBiller::Allodia => "Allodia".to_owned(),
        AllodiaBiller::Apple => "Apple".to_owned(),
        AllodiaBiller::Google => "Google Play".to_owned(),
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        AllodiaBiller::Unknown { label } => label.clone(),
    }
}

/// What a store is called on its page's button.
pub(crate) fn store_name(store: AllodiaStore) -> &'static str {
    match store {
        AllodiaStore::Apple => "Apple",
        AllodiaStore::Google => "Google Play",
        // The service never reports its own billing as a store.
        AllodiaStore::Allodia => "Allodia",
    }
}

/// A refusal's own words.
///
/// ⚠️ **A code, never a message from anywhere else**: "you have already cancelled" and "that
/// cannot change while a charge is being retried" are different things to say, and only the code
/// tells them apart.
pub(crate) fn write_failure_text(failure: AllodiaWriteFailure) -> &'static str {
    match failure {
        AllodiaWriteFailure::AlreadyCancelled => {
            l10n::settings_subscription_refused_already_cancelled()
        }
        AllodiaWriteFailure::AlreadyActive => l10n::settings_subscription_refused_already_active(),
        AllodiaWriteFailure::AlreadyOnInterval => {
            l10n::settings_subscription_refused_already_on_interval()
        }
        AllodiaWriteFailure::NotSwitchable => l10n::settings_subscription_refused_not_switchable(),
        AllodiaWriteFailure::NotFound => l10n::settings_subscription_refused_not_found(),
        AllodiaWriteFailure::Unexplained => l10n::settings_subscription_write_failed(),
    }
}

/// Minor units and an ISO 4217 code as something a reader can price.
///
/// ⚠️ **Only for what Allodia bills directly.** A store's price is the store's own formatted
/// string, drawn and never recomputed; this is the other route, where the service sends 199 and
/// `EUR` because the core carries no locale data at all.
///
/// The code rather than a symbol, in the order most of Europe writes one. This client carries no
/// currency data and will not grow a hand-kept symbol table: a borrowed symbol prices an amount in
/// the wrong money while looking entirely correct, where a code is merely plainer.
pub(crate) fn minor_units(minor: i64, currency: &str, locale: &str) -> String {
    let negative = minor < 0;
    let units = minor.unsigned_abs() / 100;
    let hundredths = minor.unsigned_abs() % 100;
    let amount = format!(
        "{}{units}{}{hundredths:02}",
        if negative { "-" } else { "" },
        decimal_separator(locale)
    );
    if currency.is_empty() {
        amount
    } else {
        format!("{amount} {currency}")
    }
}

/// What this language puts between the units and the hundredths. English is the one the catalog
/// ships that writes a point.
fn decimal_separator(locale: &str) -> char {
    if locale == "en" { '.' } else { ',' }
}

#[cfg(test)]
#[path = "allodia_subscription_facts_tests.rs"]
mod tests;
