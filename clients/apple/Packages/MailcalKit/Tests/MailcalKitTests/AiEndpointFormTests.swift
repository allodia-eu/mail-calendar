// What the own endpoint's form does with the core's record (AiEndpointEditing.swift, AiWords.swift):
// the key a save sends, what each outcome does once, and the words for a gap, a failure and a
// remedy. What a save refuses is the core's, tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

private let noGaps = AiEndpointGaps(
    address: nil, modelMissing: false, declaration: nil, keptKey: nil)

private func form(
    keyKept: Bool = false, outcome: AiEndpointOutcome? = nil
) -> AiEndpointForm {
    AiEndpointForm(
        address: "http://localhost:11434/v1", model: "mistral", declared: .euNative,
        keyKept: keyKept, save: .free(), remove: .free(), outcome: outcome)
}

private func outcome(_ serial: UInt64, _ result: AiEndpointResult) -> AiEndpointOutcome {
    AiEndpointOutcome(serial: serial, kind: .save, result: result)
}

@Suite struct AiEndpointFormTests {
    @Test func theFieldsAreTheRecordsAndTheKeyIsNeverDrawnBack() {
        let fields = AiEndpointFields(form(keyKept: true))
        #expect(fields.address == "http://localhost:11434/v1")
        #expect(fields.model == "mistral")
        #expect(fields.declared == .euNative)
        #expect(fields.key.isEmpty)
    }

    @Test func theKeyIsSentAsTyped() {
        var fields = AiEndpointFields(form(keyKept: true))
        guard case let .save(_, _, _, untouched) = fields.save else {
            Issue.record("a save is sent as a save")
            return
        }
        #expect(untouched.isEmpty, "an empty field, which leaves the kept key")
        fields.key = "sk-new"
        guard case let .save(address, model, declared, key) = fields.save else {
            Issue.record("a save is sent as a save")
            return
        }
        #expect(address == "http://localhost:11434/v1")
        #expect(model == "mistral")
        #expect(declared == .euNative)
        #expect(key == "sk-new")
    }

    @Test func anOutcomeAlreadyOnTheRecordWhenTheFormOpensIsNotActedOn() {
        var session = AiEndpointSession(form: form(outcome: outcome(3, .saved)))
        #expect(session.take(outcome(3, .saved)) == nil)
    }

    @Test func eachOutcomeIsActedOnOnce() {
        var session = AiEndpointSession(form: form())
        #expect(session.take(outcome(1, .saved)) == .adopt)
        #expect(session.take(outcome(1, .saved)) == nil)
        #expect(session.take(outcome(2, .removed)) == .adopt)
        #expect(session.take(outcome(3, .notWritten)) == .adopt)
    }

    @Test func aRefusedSaveKeepsTheEditAndOnlyAKeyNotStoredIsSaidOnTheForm() {
        var session = AiEndpointSession(form: form())
        #expect(session.take(outcome(1, .refused(gaps: noGaps))) == .keep(problem: nil))
        #expect(
            session.take(outcome(2, .keyNotStored))
                == .keep(problem: L10n.ai_endpoint_key_not_stored()))
    }

    @Test func eachGapIsSaidBesideItsField() {
        #expect(AiWords.addressGap(.missing) == L10n.ai_endpoint_gap_address_missing())
        #expect(AiWords.addressGap(.unusable) == L10n.ai_endpoint_gap_address_unusable())
        #expect(AiWords.addressGap(.needsHttps) == L10n.ai_endpoint_gap_needs_https())
        #expect(AiWords.modelGap(noGaps) == nil)
        #expect(
            AiWords.modelGap(AiEndpointGaps(
                address: nil, modelMissing: true, declaration: nil, keptKey: nil))
                == L10n.ai_endpoint_gap_model_missing())
        #expect(AiWords.declarationGap(.missing) == L10n.ai_endpoint_gap_where_missing())
        #expect(
            AiWords.declarationGap(.refused(refusal: .nonEu(mode: .euNative)))
                == L10n.ai_endpoint_gap_refused_eu_native())
        #expect(
            AiWords.declarationGap(.refused(refusal: .nonEu(mode: .euHosted)))
                == L10n.ai_endpoint_gap_refused_eu_hosted())
        #expect(
            AiWords.declarationGap(.refused(refusal: .euHosted))
                == L10n.ai_endpoint_gap_refused_eu_native(),
            "only an EU-native limit refuses an endpoint run in the EU from outside it")
    }

    @Test func aRefusalIsWordedByTheLimitItIsUnder() {
        #expect(AiWords.failure(.refused(refusal: .euHosted)) == L10n.ai_failure_refused_eu_native())
        #expect(
            AiWords.failure(.refused(refusal: .unknown(mode: .euHosted)))
                == L10n.ai_failure_refused_eu_hosted())
    }

    @Test func theKeyFieldSaysWhatTheSaveWouldDoToTheKeptKey() {
        #expect(AiWords.keyHint(.carried) == L10n.ai_endpoint_key_stored())
        #expect(AiWords.keyHint(.dropped) == L10n.ai_endpoint_key_dropped())
        #expect(AiWords.keyHint(nil) == L10n.ai_endpoint_key_hint())
    }

    @Test func everyFailureAndRemedyHasWords() {
        let failures: [AiFailure] = [
            .relaySignIn, .endpointKeyRefused, .nowhereToSend, .unreachable, .serverError,
            .tooManyRequests, .unreadable, .outOfCredits, .notEntitled,
            .noSentFolder(anotherAccount: true), .nothingToLearn, .sampleTooLarge, .noStyle,
            .noStyleForAccount, .messageGone,
        ]
        #expect(Set(failures.map(AiWords.failure)).count == failures.count)
        let remedies: [AiRemedy] = [
            .signInToAllodia, .openAiEndpoint, .tryAgain, .chooseAccount, .chooseRange,
            .learnStyle, .chooseStyle,
        ]
        #expect(Set(remedies.map(AiWords.remedy)).count == remedies.count)
    }

    @Test func undeclaredIsNeverOfferedAsAnAnswer() {
        #expect(AiWords.declarations.map(\.0) == [.euNative, .euHosted, .nonEu])
    }
}
