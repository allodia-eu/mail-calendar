//! The words for what the writing assistant's records say (`docs/ai.md` rule 27): a failure, its
//! remedy, a field's gap, a range, a date. A record decides; this file only names.

use mailcal_bindings::{
    AiEndpointAddressGap, AiEndpointDeclarationGap, AiFailure, AiRemedy, JurisdictionClass,
    JurisdictionLimit, JurisdictionRefusal, StyleLearnRange, StyleSentReach, WritingStyleFacts,
    WritingStyleFrequency, WritingStyleRefusal, WritingStyleTopic,
};

use crate::{l10n, ui::timestamps};

pub(in crate::ui) fn failure(failure: AiFailure) -> &'static str {
    match failure {
        AiFailure::RelaySignIn => l10n::ai_failure_relay_sign_in(),
        AiFailure::EndpointKeyRefused => l10n::ai_failure_key_refused(),
        AiFailure::Refused { refusal } => match refusal {
            JurisdictionRefusal::EuHosted
            | JurisdictionRefusal::NonEu {
                mode: JurisdictionLimit::EuNative,
            } => l10n::ai_failure_refused_eu_native(),
            JurisdictionRefusal::NonEu {
                mode: JurisdictionLimit::EuHosted,
            } => l10n::ai_failure_refused_eu_hosted(),
            JurisdictionRefusal::Unknown { .. } => l10n::ai_failure_refused_undeclared(),
        },
        AiFailure::NowhereToSend => l10n::ai_failure_nowhere_to_send(),
        AiFailure::Unreachable => l10n::ai_failure_unreachable(),
        AiFailure::ServerError => l10n::ai_failure_server_error(),
        AiFailure::TooManyRequests => l10n::ai_failure_too_many_requests(),
        AiFailure::Unreadable => l10n::ai_failure_unreadable(),
        AiFailure::OutOfCredits => l10n::ai_failure_out_of_credits(),
        AiFailure::NotEntitled => l10n::ai_failure_not_entitled(),
        AiFailure::NoSentFolder { .. } => l10n::ai_failure_no_sent_folder(),
        AiFailure::NothingToLearn => l10n::ai_failure_nothing_to_learn(),
        AiFailure::SampleTooLarge => l10n::ai_failure_sample_too_large(),
        AiFailure::NoStyle => l10n::ai_failure_no_style(),
        AiFailure::NoStyleForAccount => l10n::ai_failure_no_style_for_account(),
        AiFailure::MessageGone => l10n::ai_failure_message_gone(),
    }
}

pub(in crate::ui) fn remedy(remedy: AiRemedy) -> &'static str {
    match remedy {
        AiRemedy::SignInToAllodia => l10n::ai_remedy_sign_in(),
        AiRemedy::OpenAiEndpoint => l10n::ai_remedy_open_endpoint(),
        AiRemedy::TryAgain => l10n::ai_remedy_try_again(),
        AiRemedy::ChooseAccount => l10n::ai_remedy_choose_account(),
        AiRemedy::ChooseRange => l10n::ai_remedy_choose_range(),
        AiRemedy::LearnStyle => l10n::ai_remedy_learn_style(),
        AiRemedy::ChooseStyle => l10n::ai_remedy_choose_style(),
    }
}

pub(super) fn address_gap(gap: AiEndpointAddressGap) -> &'static str {
    match gap {
        AiEndpointAddressGap::Missing => l10n::ai_endpoint_gap_address_missing(),
        AiEndpointAddressGap::Unusable => l10n::ai_endpoint_gap_address_unusable(),
        AiEndpointAddressGap::NeedsHttps => l10n::ai_endpoint_gap_needs_https(),
    }
}

pub(super) fn declaration_gap(gap: AiEndpointDeclarationGap) -> &'static str {
    match gap {
        AiEndpointDeclarationGap::Missing => l10n::ai_endpoint_gap_where_missing(),
        AiEndpointDeclarationGap::Refused { refusal } => match refusal {
            JurisdictionRefusal::EuHosted
            | JurisdictionRefusal::NonEu {
                mode: JurisdictionLimit::EuNative,
            } => l10n::ai_endpoint_gap_refused_eu_native(),
            JurisdictionRefusal::NonEu {
                mode: JurisdictionLimit::EuHosted,
            } => l10n::ai_endpoint_gap_refused_eu_hosted(),
            JurisdictionRefusal::Unknown { .. } => l10n::ai_endpoint_gap_where_missing(),
        },
    }
}

/// The declarations the form offers, in order. Not saying is the absence of a pick, never one of
/// these: the gate admits an unclassified endpoint only under a mode no client offers.
pub(super) const DECLARATIONS: [JurisdictionClass; 3] = [
    JurisdictionClass::EuNative,
    JurisdictionClass::EuHosted,
    JurisdictionClass::NonEu,
];

pub(super) fn declaration(class: JurisdictionClass) -> &'static str {
    match class {
        JurisdictionClass::EuNative => l10n::ai_endpoint_where_eu_native(),
        JurisdictionClass::EuHosted => l10n::ai_endpoint_where_eu_hosted(),
        JurisdictionClass::NonEu | JurisdictionClass::Unknown => l10n::ai_endpoint_where_non_eu(),
    }
}

pub(super) fn refusal(refusal: WritingStyleRefusal) -> &'static str {
    match refusal {
        WritingStyleRefusal::BlankName => l10n::writing_style_refused_blank_name(),
        WritingStyleRefusal::Gone => l10n::writing_style_refused_gone(),
    }
}

/// A day, from `YYYY-MM-DD` or an instant, as the app's language writes one.
pub(super) fn day(raw: &str) -> String {
    timestamps::account_date(raw, l10n::active_locale()).unwrap_or_else(|| raw.to_owned())
}

/// A moment in seconds since the epoch, as its day in UTC, which is how the core counts days.
pub(super) fn day_of(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds)
        .map_or_else(|_| seconds.to_string(), |instant| day(&instant.to_string()))
}

/// Languages by their own names, in the order given.
pub(super) fn languages(codes: &[String]) -> String {
    codes
        .iter()
        .map(|code| l10n::language_name(code))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The line under a style's name: what it was learned from, when, and in which languages.
pub(super) fn facts(facts: &WritingStyleFacts) -> String {
    let learned = facts
        .learned_at
        .map(|at| l10n::writing_style_learned_from(i64::from(facts.messages), &day_of(at)));
    let languages = (!facts.languages.is_empty())
        .then(|| l10n::writing_style_languages(&languages(&facts.languages)));
    learned
        .into_iter()
        .chain(languages)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A range as chosen: a preset by its name, a range of the person's own by its two days.
pub(super) fn range_chosen(range: &StyleLearnRange) -> String {
    match range {
        StyleLearnRange::Between { since, until } => format!("{} – {}", day(since), day(until)),
        other => self::range(other).to_owned(),
    }
}

pub(super) fn range(range: &StyleLearnRange) -> &'static str {
    match range {
        StyleLearnRange::Everything => l10n::learn_range_all(),
        StyleLearnRange::UntilEndOfLastYear => l10n::learn_range_until_last_year(),
        StyleLearnRange::Between { .. } => l10n::learn_range_between(),
    }
}

/// How far back the device holds the account's mail.
pub(super) fn reach(reach: &StyleSentReach) -> String {
    match reach {
        StyleSentReach::Everything => l10n::learn_report_horizon_all().to_owned(),
        StyleSentReach::Since { day: since } => l10n::learn_report_horizon(&day(since)),
    }
}

/// The run's progress: a fraction once the run knows how many requests it makes.
pub(super) fn progress(done: u32, total: u32) -> String {
    if total == 0 {
        l10n::learn_progress_starting().to_owned()
    } else {
        l10n::learn_progress(&done.to_string(), &total.to_string())
    }
}

pub(super) fn frequency(frequency: WritingStyleFrequency) -> &'static str {
    match frequency {
        WritingStyleFrequency::Mostly => l10n::reveal_frequency_mostly(),
        WritingStyleFrequency::Often => l10n::reveal_frequency_often(),
        WritingStyleFrequency::Sometimes => l10n::reveal_frequency_sometimes(),
    }
}

pub(super) fn topic(topic: WritingStyleTopic) -> &'static str {
    match topic {
        WritingStyleTopic::Register => l10n::reveal_register(),
        WritingStyleTopic::Structure => l10n::reveal_structure(),
        WritingStyleTopic::Moves => l10n::reveal_moves(),
        WritingStyleTopic::Punctuation => l10n::reveal_punctuation(),
    }
}
