//! Every failure, remedy, refusal and endpoint gap a client words has its message in the catalog,
//! so no client adds one of its own, and every such message in the catalog is one of them. Each
//! match below is exhaustive: a variant added without a message does not compile. The catalog's
//! own validator holds the other locales to English.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    AiEndpointAddressGap, AiEndpointDeclarationGap, AiEndpointKeptKey, AiFailure, AiRemedy,
    JurisdictionLimit, JurisdictionRefusal,
};

const CATALOG: &str = include_str!("../../../messages/en.json");

/// The words a refusal by the gate is said in, on a failure (`failure`) or beside the form's
/// declaration (`gap`).
fn refusal_key(refusal: JurisdictionRefusal, failure: bool) -> &'static str {
    let mode = match refusal {
        JurisdictionRefusal::EuHosted => JurisdictionLimit::EuNative,
        JurisdictionRefusal::NonEu { mode } | JurisdictionRefusal::Unknown { mode } => mode,
    };
    match (refusal, mode, failure) {
        (JurisdictionRefusal::Unknown { .. }, _, true) => "ai_failure_refused_undeclared",
        (_, JurisdictionLimit::EuNative, true) => "ai_failure_refused_eu_native",
        (_, JurisdictionLimit::EuHosted, true) => "ai_failure_refused_eu_hosted",
        (_, JurisdictionLimit::EuNative, false) => "ai_endpoint_gap_refused_eu_native",
        (_, JurisdictionLimit::EuHosted, false) => "ai_endpoint_gap_refused_eu_hosted",
    }
}

fn failure_key(failure: AiFailure) -> &'static str {
    match failure {
        AiFailure::RelaySignIn => "ai_failure_relay_sign_in",
        AiFailure::EndpointKeyRefused => "ai_failure_key_refused",
        AiFailure::Refused { refusal } => refusal_key(refusal, true),
        AiFailure::NowhereToSend => "ai_failure_nowhere_to_send",
        AiFailure::Unreachable => "ai_failure_unreachable",
        AiFailure::ServerError => "ai_failure_server_error",
        AiFailure::TooManyRequests => "ai_failure_too_many_requests",
        AiFailure::Unreadable => "ai_failure_unreadable",
        AiFailure::OutOfCredits => "ai_failure_out_of_credits",
        AiFailure::NotEntitled => "ai_failure_not_entitled",
        AiFailure::NoSentFolder { .. } => "ai_failure_no_sent_folder",
        AiFailure::NothingToLearn => "ai_failure_nothing_to_learn",
        AiFailure::SampleTooLarge => "ai_failure_sample_too_large",
        AiFailure::NoStyle => "ai_failure_no_style",
        AiFailure::NoStyleForAccount => "ai_failure_no_style_for_account",
        AiFailure::MessageGone => "ai_failure_message_gone",
    }
}

fn remedy_key(remedy: AiRemedy) -> &'static str {
    match remedy {
        AiRemedy::SignInToAllodia => "ai_remedy_sign_in",
        AiRemedy::OpenAiEndpoint => "ai_remedy_open_endpoint",
        AiRemedy::TryAgain => "ai_remedy_try_again",
        AiRemedy::ChooseAccount => "ai_remedy_choose_account",
        AiRemedy::ChooseRange => "ai_remedy_choose_range",
        AiRemedy::LearnStyle => "ai_remedy_learn_style",
        AiRemedy::ChooseStyle => "ai_remedy_choose_style",
    }
}

fn address_gap_key(gap: AiEndpointAddressGap) -> &'static str {
    match gap {
        AiEndpointAddressGap::Missing => "ai_endpoint_gap_address_missing",
        AiEndpointAddressGap::Unusable => "ai_endpoint_gap_address_unusable",
        AiEndpointAddressGap::NeedsHttps => "ai_endpoint_gap_needs_https",
    }
}

fn declaration_gap_key(gap: AiEndpointDeclarationGap) -> &'static str {
    match gap {
        AiEndpointDeclarationGap::Missing => "ai_endpoint_gap_where_missing",
        AiEndpointDeclarationGap::Refused { refusal } => refusal_key(refusal, false),
    }
}

fn kept_key_key(kept: AiEndpointKeptKey) -> &'static str {
    match kept {
        AiEndpointKeptKey::Carried => "ai_endpoint_key_stored",
        AiEndpointKeptKey::Dropped => "ai_endpoint_key_dropped",
    }
}

fn every_refusal() -> Vec<JurisdictionRefusal> {
    let modes = [JurisdictionLimit::EuHosted, JurisdictionLimit::EuNative];
    std::iter::once(JurisdictionRefusal::EuHosted)
        .chain(modes.map(|mode| JurisdictionRefusal::NonEu { mode }))
        .chain(modes.map(|mode| JurisdictionRefusal::Unknown { mode }))
        .collect()
}

/// Every key the words above name, one of each variant.
fn named() -> BTreeSet<&'static str> {
    let failures = [
        AiFailure::RelaySignIn,
        AiFailure::EndpointKeyRefused,
        AiFailure::NowhereToSend,
        AiFailure::Unreachable,
        AiFailure::ServerError,
        AiFailure::TooManyRequests,
        AiFailure::Unreadable,
        AiFailure::OutOfCredits,
        AiFailure::NotEntitled,
        AiFailure::NoSentFolder {
            another_account: true,
        },
        AiFailure::NothingToLearn,
        AiFailure::SampleTooLarge,
        AiFailure::NoStyle,
        AiFailure::NoStyleForAccount,
        AiFailure::MessageGone,
    ];
    let remedies = [
        AiRemedy::SignInToAllodia,
        AiRemedy::OpenAiEndpoint,
        AiRemedy::TryAgain,
        AiRemedy::ChooseAccount,
        AiRemedy::ChooseRange,
        AiRemedy::LearnStyle,
        AiRemedy::ChooseStyle,
    ];
    let addresses = [
        AiEndpointAddressGap::Missing,
        AiEndpointAddressGap::Unusable,
        AiEndpointAddressGap::NeedsHttps,
    ];
    let refusals = every_refusal();
    failures
        .into_iter()
        .chain(
            refusals
                .iter()
                .map(|&refusal| AiFailure::Refused { refusal }),
        )
        .map(failure_key)
        .chain(remedies.map(remedy_key))
        .chain(addresses.map(address_gap_key))
        .chain(
            std::iter::once(AiEndpointDeclarationGap::Missing)
                .chain(
                    refusals
                        .iter()
                        .map(|&refusal| AiEndpointDeclarationGap::Refused { refusal }),
                )
                .map(declaration_gap_key),
        )
        .chain([AiEndpointKeptKey::Carried, AiEndpointKeptKey::Dropped].map(kept_key_key))
        .chain(["ai_endpoint_gap_model_missing"])
        .collect()
}

#[test]
fn every_failure_remedy_and_gap_has_its_words_in_the_catalog() {
    let catalog: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(CATALOG).expect("the catalog is JSON");
    let named = named();
    for key in &named {
        assert!(catalog.contains_key(*key), "{key} is not in the catalog");
    }
    let worded: BTreeSet<&str> = catalog
        .keys()
        .map(String::as_str)
        .filter(|key| {
            ["ai_failure_", "ai_remedy_", "ai_endpoint_gap_"]
                .iter()
                .any(|prefix| key.starts_with(prefix))
        })
        .collect();
    let unnamed: Vec<&&str> = worded.difference(&named).collect();
    assert!(unnamed.is_empty(), "words no variant says: {unnamed:?}");
}
