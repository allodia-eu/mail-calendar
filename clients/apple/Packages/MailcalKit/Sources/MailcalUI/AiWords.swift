// The catalog's words for the writing assistant's records (docs/ai.md rule 27): a failure, its
// remedy, and what the own endpoint's form says beside a field. The core decides each; this only
// words it, so a server's own sentence is never shown. Plain functions, so the unit suite reads
// them without a screen.

import MailcalBindings

enum AiWords {
    /// Why learning or drafting did not happen.
    static func failure(_ failure: AiFailure) -> String {
        switch failure {
        case .relaySignIn: L10n.ai_failure_relay_sign_in()
        case .endpointKeyRefused: L10n.ai_failure_key_refused()
        case let .refused(mode, _): refusedFailure(mode)
        case .nowhereToSend: L10n.ai_failure_nowhere_to_send()
        case .unreachable: L10n.ai_failure_unreachable()
        case .serverError: L10n.ai_failure_server_error()
        case .tooManyRequests: L10n.ai_failure_too_many_requests()
        case .unreadable: L10n.ai_failure_unreadable()
        case .outOfCredits: L10n.ai_failure_out_of_credits()
        case .notEntitled: L10n.ai_failure_not_entitled()
        case .noSentFolder: L10n.ai_failure_no_sent_folder()
        case .nothingToLearn: L10n.ai_failure_nothing_to_learn()
        case .sampleTooLarge: L10n.ai_failure_sample_too_large()
        case .noStyle: L10n.ai_failure_no_style()
        case .noStyleForAccount: L10n.ai_failure_no_style_for_account()
        case .messageGone: L10n.ai_failure_message_gone()
        }
    }

    /// The one action a failure offers, as its control's title.
    static func remedy(_ remedy: AiRemedy) -> String {
        switch remedy {
        case .signInToAllodia: L10n.ai_remedy_sign_in()
        case .openAiEndpoint: L10n.ai_remedy_open_endpoint()
        case .tryAgain: L10n.ai_remedy_try_again()
        case .chooseAccount: L10n.ai_remedy_choose_account()
        case .chooseRange: L10n.ai_remedy_choose_range()
        case .learnStyle: L10n.ai_remedy_learn_style()
        case .chooseStyle: L10n.ai_remedy_choose_style()
        }
    }

    /// What is wrong with the address, said beside it.
    static func addressGap(_ gap: AiEndpointAddressGap) -> String {
        switch gap {
        case .missing: L10n.ai_endpoint_gap_address_missing()
        case .unusable: L10n.ai_endpoint_gap_address_unusable()
        case .needsHttps: L10n.ai_endpoint_gap_needs_https()
        }
    }

    /// What is wrong with the declaration, said beside it.
    static func declarationGap(_ gap: AiEndpointDeclarationGap) -> String {
        switch gap {
        case .missing: L10n.ai_endpoint_gap_where_missing()
        case let .refused(mode, _): refusedGap(mode)
        }
    }

    /// The model field's gap, when it has one.
    static func modelGap(_ gaps: AiEndpointGaps) -> String? {
        gaps.modelMissing ? L10n.ai_endpoint_gap_model_missing() : nil
    }

    /// Where an endpoint runs, as the person declares it. `unknown` is never offered: leaving the
    /// choice unmade is how a person declares nothing.
    static var declarations: [(JurisdictionClass, String)] {
        [
            (.euNative, L10n.ai_endpoint_where_eu_native()),
            (.euHosted, L10n.ai_endpoint_where_eu_hosted()),
            (.nonEu, L10n.ai_endpoint_where_non_eu()),
        ]
    }

    /// The gate refuses only under a mode narrower than `all`, so `all` never reaches these two;
    /// it takes the wider mode's sentence rather than an empty one.
    private static func refusedFailure(_ mode: JurisdictionMode) -> String {
        mode == .euNative ? L10n.ai_failure_refused_eu_native() : L10n.ai_failure_refused_eu_hosted()
    }

    private static func refusedGap(_ mode: JurisdictionMode) -> String {
        mode == .euNative
            ? L10n.ai_endpoint_gap_refused_eu_native() : L10n.ai_endpoint_gap_refused_eu_hosted()
    }
}
