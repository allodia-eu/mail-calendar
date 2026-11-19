// What Settings → Writing style does with the core's records that is not drawing
// (WritingStyleEditing.swift, WritingStyleWords.swift): which outcome a surface acts on and once,
// where a remedy goes, how a stage is keyed, and how a day the core names is shown and picked.
// What the records say is the core's, tested there.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

private func outcome(
    _ serial: UInt64, _ kind: WritingStyleChangeKind, style: String? = "s1",
    account: String? = nil, _ result: WritingStyleResult = .done
) -> WritingStyleOutcome {
    WritingStyleOutcome(serial: serial, kind: kind, style: style, accountId: account, result: result)
}

private func page(outcome: WritingStyleOutcome?) -> WritingStylePage {
    WritingStylePage(
        styles: [], accounts: [], learn: .free(), learnBlocked: nil, run: nil, outcome: outcome)
}

@Suite struct WritingStyleSessionTests {
    @Test func thePageSaysARefusedSlotOnceAndNothingElse() {
        var session = WritingStylePageSession(page: nil)
        let refused = outcome(1, .assign, style: nil, account: "a", .refused(reason: .gone))
        #expect(session.take(refused) == L10n.writing_style_refused_gone())
        #expect(session.take(refused) == nil, "an outcome is said once")
        #expect(session.take(outcome(2, .assign, style: nil, account: "a")) == nil)
        #expect(session.take(outcome(3, .rename, .refused(reason: .blankName))) == nil,
                "a rename is the detail's to say")
    }

    @Test func anOutcomeOnThePageWhenASheetOpensIsNotItsConcern() {
        var session = WritingStyleSheetSession(
            style: "s1", page: page(outcome: outcome(4, .forget)))
        #expect(session.take(outcome(4, .forget)) == nil)
    }

    @Test func aSheetClosesOnLeavingAndStaysOnAnEdit() {
        var session = WritingStyleSheetSession(style: "s1", page: nil)
        #expect(session.take(outcome(1, .rename)) == .stay)
        #expect(session.take(outcome(2, .notes)) == .stay)
        #expect(session.take(outcome(3, .saveReveal)) == .close)
        #expect(session.take(outcome(4, .closeReveal, .notWritten)) == .close)
        #expect(session.take(outcome(5, .forget)) == .close)
    }

    @Test func aSheetSaysItsOwnRefusalAndIgnoresAnotherStyles() {
        var session = WritingStyleSheetSession(style: "s1", page: nil)
        #expect(session.take(outcome(1, .saveReveal, style: "s2")) == nil)
        #expect(
            session.take(outcome(2, .saveReveal, .refused(reason: .blankName)))
                == .refused(L10n.writing_style_refused_blank_name()))
    }
}

@Suite struct WritingStyleRemedyTests {
    @Test func aRemedyThatMovesTheFlowIsTheFlowsAndTheRestOpenWhatTheyName() {
        #expect(WritingStyleRemedyRoute(.chooseAccount) == .flow)
        #expect(WritingStyleRemedyRoute(.chooseRange) == .flow)
        #expect(WritingStyleRemedyRoute(.tryAgain) == .flow)
        #expect(WritingStyleRemedyRoute(.signInToAllodia) == .allodiaSignIn)
        #expect(WritingStyleRemedyRoute(.openAiEndpoint) == .endpointForm)
        #expect(WritingStyleRemedyRoute(.learnStyle) == .learn)
        #expect(WritingStyleRemedyRoute(.chooseStyle) == .accountStyles)
    }

    @Test func aStageIsKeyedByWhichItIsNotWhatItHolds() {
        let early = StyleLearningStage.learning(done: 0, total: 0)
        let later = StyleLearningStage.learning(done: 2, total: 5)
        #expect(StyleLearningStageKey(early) == StyleLearningStageKey(later))
        #expect(StyleLearningStageKey(.learned(style: "s1")) == .learned)
    }

    @Test func aRangeOfThePersonsOwnIsOnePresetWhateverItsDays() {
        let one = StyleLearnRange.between(since: "2026-01-01", until: "2026-02-01")
        #expect(one.samePreset(as: .between(since: "2025-01-01", until: "2025-12-31")))
        #expect(!one.samePreset(as: .everything))
        #expect(!StyleLearnRange.everything.samePreset(as: nil))
    }
}

@Suite struct WritingStyleWordsTests {
    private let english = Locale(identifier: "en_GB")

    @Test func aDayIsReadAndPickedInUTC() throws {
        let date = try #require(WritingStyleWords.dayDate("2026-08-19"))
        #expect(WritingStyleWords.dayText(date) == "2026-08-19")
        #expect(WritingStyleWords.day("2026-08-19", locale: english) == "19 August 2026")
        #expect(WritingStyleWords.dayDate("not a day") == nil)
        #expect(WritingStyleWords.day("not a day", locale: english) == "not a day")
    }

    @Test func progressIsAFractionOnlyOnceTheRunKnowsItsTotal() {
        #expect(WritingStyleWords.progress(done: 0, total: 0) == L10n.learn_progress_starting())
        #expect(
            WritingStyleWords.progress(done: 1, total: 3)
                == L10n.learn_progress(done: "1", total: "3"))
    }

    @Test func theReachSaysHowFarBackTheDeviceHolds() {
        #expect(WritingStyleWords.reach(.everything) == L10n.learn_report_horizon_all())
        #expect(
            WritingStyleWords.reach(.since(day: "2026-08-19"))
                == L10n.learn_report_horizon(date: WritingStyleWords.day("2026-08-19")))
    }

    @Test func theConsentNamesTheRelayOrTheEndpointsHost() {
        #expect(WritingStyleWords.consent(.relay) == L10n.learn_consent_relay())
        #expect(
            WritingStyleWords.consent(.ownEndpoint(host: "localhost"))
                == L10n.learn_consent_own(host: "localhost"))
    }

    @Test func learnHeldSaysWhy() {
        #expect(WritingStyleWords.blocked(.noMailAccount) == L10n.writing_style_learn_no_account())
        #expect(
            WritingStyleWords.blocked(.cannotSend(failure: .nowhereToSend, remedy: .openAiEndpoint))
                == L10n.ai_failure_nowhere_to_send())
    }

    @Test func forgettingNamesTheAccountsThatDraftInTheStyle() {
        #expect(WritingStyleWords.forgetMessage([]) == L10n.writing_style_forget_message())
        let named = WritingStyleWords.forgetMessage(["a@test.local"], locale: english)
        #expect(named.hasPrefix(L10n.writing_style_forget_accounts(accounts: "a@test.local")))
        #expect(named.hasSuffix(L10n.writing_style_forget_message()))
    }

    @Test func everyRevealPageHasATitle() {
        let pages: [WritingStyleRevealPage] = [.read, .letter, .habits, .voice, .phrases, .name]
        #expect(Set(pages.map(WritingStyleWords.page)).count == pages.count)
    }

    @Test func aLanguageIsNamedInTheReadersLanguage() {
        #expect(WritingStyleWords.language("nl", locale: english) == "Dutch")
        #expect(WritingStyleWords.languages(["en", "nl"], locale: english) == "English and Dutch")
    }
}
