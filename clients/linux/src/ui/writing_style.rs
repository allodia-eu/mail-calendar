//! What Writing style says, decided without a widget (`docs/ai.md`).
//!
//! The copy for each failure, the credits line, a style's summary, the composer's few rules, and
//! the snapshot the model pushes into an open Settings window. Kept free of GTK so each rule is a
//! plain test; where a learning run stands is [`super::writing_style_flow`], what the reveal draws
//! is [`super::writing_style_reveal`], and the screens that draw them are the Writing style
//! settings category and the composer's Draft a reply control.

use mailcal_bindings::{
    AiRoute, CreditBalance, JurisdictionMode, OwnEndpointError, WritingStyleFailure,
    WritingStyleRow, WritingStyleSnapshot,
};

use super::{allodia_subscription_facts::decimal_separator, timestamps};
use crate::l10n;

/// A failure in the person's words. Never a server's sentence: the variant is all a client has.
///
/// `route` decides one of them. A refused sign-in through the relay is fixed by signing in to the
/// Allodia account again; a refused own endpoint by its key, which is the person's own.
pub(crate) fn failure_text(failure: &WritingStyleFailure, route: Option<AiRoute>) -> String {
    match failure {
        WritingStyleFailure::Unavailable => l10n::ai_error_unavailable().to_owned(),
        WritingStyleFailure::Busy => l10n::ai_error_busy().to_owned(),
        WritingStyleFailure::NoSentFolder => l10n::ai_error_no_sent_folder().to_owned(),
        WritingStyleFailure::NothingToLearn => l10n::learn_report_nothing().to_owned(),
        WritingStyleFailure::NoStyle => l10n::ai_error_no_style().to_owned(),
        WritingStyleFailure::NotFound => l10n::ai_error_not_found().to_owned(),
        WritingStyleFailure::Refused { mode, .. } => refused_text(*mode).to_owned(),
        WritingStyleFailure::OutOfCredits => l10n::ai_error_out_of_credits().to_owned(),
        WritingStyleFailure::NotEntitled => l10n::ai_error_not_entitled().to_owned(),
        WritingStyleFailure::Unauthorized => unauthorized_text(route).to_owned(),
        WritingStyleFailure::RateLimited => l10n::ai_error_rate_limited().to_owned(),
        WritingStyleFailure::Unreachable => l10n::ai_error_unreachable().to_owned(),
        WritingStyleFailure::Status { code } => l10n::ai_error_status(&code.to_string()),
        WritingStyleFailure::Malformed => l10n::ai_error_malformed().to_owned(),
        WritingStyleFailure::Cancelled => l10n::ai_error_cancelled().to_owned(),
    }
}

fn unauthorized_text(route: Option<AiRoute>) -> &'static str {
    if route == Some(AiRoute::Relay) {
        l10n::ai_error_sign_in_again()
    } else {
        l10n::ai_error_key_refused()
    }
}

/// Why the gate would refuse, by the mode in force. `All` admits everything and so never refuses;
/// it shares the stricter sentence rather than inventing one nobody can reach.
pub(crate) fn refused_text(mode: JurisdictionMode) -> &'static str {
    match mode {
        JurisdictionMode::EuHosted => l10n::writing_style_refused_eu_hosted(),
        JurisdictionMode::EuNative | JurisdictionMode::All => {
            l10n::writing_style_refused_eu_native()
        }
    }
}

/// Why the own endpoint was not saved.
pub(crate) fn endpoint_error_text(error: &OwnEndpointError) -> &'static str {
    match error {
        OwnEndpointError::InvalidUrl => l10n::ai_endpoint_error_address(),
        OwnEndpointError::NotHttps => l10n::ai_endpoint_error_https(),
        OwnEndpointError::NoModel => l10n::ai_endpoint_error_model(),
        OwnEndpointError::Keystore(_) => l10n::ai_endpoint_error_keystore(),
    }
}

/// The balance a credits line may state: only one Allodia's relay reported, and only while
/// requests still go through it.
pub(crate) fn shown_credits(snapshot: &WritingStyleSnapshot) -> Option<&CreditBalance> {
    snapshot
        .balance
        .as_ref()
        .filter(|_| snapshot.route == Some(AiRoute::Relay))
}

/// "12.5 credits left, as of 14:05": the balance to at most one decimal in the reader's
/// punctuation, and the moment the relay reported it the way the message list dates a row.
pub(crate) fn credits_line(balance: &CreditBalance, zone: &str, locale: &str) -> String {
    let as_of = jiff::Timestamp::from_second(balance.as_of)
        .map(|at| timestamps::relative_date(&at.to_string(), zone))
        .unwrap_or_default();
    l10n::writing_style_credits(&format_credits(balance.credits, locale), &as_of)
}

/// A balance to at most one decimal. Whole numbers drop the decimal; a balance the relay cannot
/// have sent (negative, not a number) reads as nothing left rather than as `-0`.
pub(crate) fn format_credits(credits: f64, locale: &str) -> String {
    let rounded = format!("{:.1}", credits.max(0.0));
    let shown = rounded.strip_suffix(".0").unwrap_or(&rounded);
    let separator = decimal_separator(locale);
    shown
        .chars()
        .map(|character| {
            if character == '.' {
                separator
            } else {
                character
            }
        })
        .collect()
}

/// Languages by their own names ("Nederlands", "English"), in the order given.
pub(crate) fn languages_text(codes: &[String]) -> String {
    codes
        .iter()
        .map(|code| l10n::language_name(code.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The library row's second line: what the style was learned from and when, and its languages.
/// A style from another device may carry no date, and then says only what it covers.
pub(crate) fn style_summary(style: &WritingStyleRow, zone: &str, locale: &str) -> String {
    let learned = Some(style.learned_at)
        .filter(|at| *at != 0)
        .and_then(|at| timestamps::local_day(at, zone, locale))
        .map(|day| l10n::writing_style_learned_from(i64::from(style.messages), &day));
    let languages = (!style.languages.is_empty())
        .then(|| l10n::writing_style_languages(&languages_text(&style.languages)));
    learned
        .into_iter()
        .chain(languages)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The From account `draft_reply` is told about: only one the person moved to. The message's own
/// account is where the core looks when it is told nothing.
pub(crate) fn changed_sender(selected: Option<&str>, opened_with: Option<&str>) -> Option<String> {
    selected
        .filter(|selected| Some(*selected) != opened_with)
        .map(str::to_owned)
}

/// What the person asked the reply to say, or nothing when the field holds only space.
pub(crate) fn intent_of(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// Whether a draft may go in without asking, from the editor's answer to `composerLeadHasText()`.
///
/// Only a definite "nothing written" skips the question. An answer that did not arrive cannot say
/// the lead is empty, and a question costs a click where a wrong guess costs what they wrote.
pub(crate) fn lead_is_empty(answer: Option<&str>) -> bool {
    answer.is_some_and(|answer| answer.trim() == "false")
}

/// The editor call that puts a draft above the signature and the quote. Both values travel as JSON
/// string literals, so a draft holding a quote mark or a closing tag stays data.
pub(crate) fn draft_script(text: &str, draft_id: &str) -> String {
    format!(
        "window.setComposerDraftText({}, {});",
        serde_json::json!(text),
        serde_json::json!(draft_id)
    )
}

/// The Writing style snapshot the model last pulled, and how many it has pulled, so an open
/// Settings window redraws once per new one.
#[derive(Debug, Default)]
pub(crate) struct WritingStyleFeed {
    generation: u64,
    snapshot: Option<WritingStyleSnapshot>,
}

impl WritingStyleFeed {
    /// Takes the snapshot a `Surface::WritingStyle` signal pulled. Answers whether AI became
    /// available or stopped being so: the category comes or goes with it, and only a rebuild
    /// adds or removes a sidebar row.
    pub(crate) fn receive(&mut self, snapshot: WritingStyleSnapshot) -> bool {
        let was = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.route.is_some());
        let now = snapshot.route.is_some();
        self.snapshot = Some(snapshot);
        self.generation = self.generation.wrapping_add(1);
        was != now
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn snapshot(&self) -> Option<&WritingStyleSnapshot> {
        self.snapshot.as_ref()
    }
}

#[cfg(test)]
#[path = "writing_style_tests.rs"]
mod tests;
