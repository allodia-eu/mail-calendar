// What Settings → Writing style does with the core's records that is not drawing
// (WritingStyleEditing.swift, WritingStyleWords.swift, SettingsWritingStyle.swift): which outcome
// a surface acts on and once, what a sheet closed by a swipe sends, when the detail's Save is held,
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

@Suite struct WritingStyleSheetTests {
    private func detail(name: String, rename: ControlState = .free()) -> WritingStyleDetail {
        WritingStyleDetail(
            id: "s1",
            facts: WritingStyleFacts(
                messages: 12, learnedFrom: "a@test.local", since: nil, learnedAt: nil,
                languages: ["en"]),
            sections: [], languages: [], name: name, notes: "", rename: rename,
            notesControl: .free(), forget: .free(), draftingAccounts: [])
    }

    @Test func aRevealClosedBySwipeIsLeftAsItsCloseLeavesIt() {
        let reveal = OpenedStyle(id: "s1", opens: .reveal)
        guard case let .closeReveal(style)? = reveal.dismissed else {
            Issue.record("a reveal swiped away is not counted as shown")
            return
        }
        #expect(style == "s1")
        #expect(OpenedStyle(id: "s1", opens: .detail).dismissed == nil)
    }

    @Test func theDetailsSaveIsHeldOnlyByTheRecordAndTheCoresRefusal() {
        let work = detail(name: "Work")
        #expect(work.nameSave("Work").enabled, "the name it already has is no refusal")
        #expect(work.nameSave("Home").enabled)
        #expect(!work.nameSave("  ").enabled)
        let held = detail(name: "Work", rename: .free(false))
        #expect(!held.nameSave("Home").enabled)
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

    @Test func theTrailNamesTheStagesTheHostPushes() {
        #expect(StyleLearningStageKey(StyleLearningTrailStage.chooseAccount) == .chooseAccount)
        #expect(StyleLearningStageKey(StyleLearningTrailStage.chooseRange) == .chooseRange)
        #expect(StyleLearningStageKey(StyleLearningTrailStage.review) == .review)
    }

    @Test func aRangeOfThePersonsOwnIsOnePresetWhateverItsDays() {
        let one = StyleLearnRange.between(since: "2026-01-01", until: "2026-02-01")
        #expect(one.samePreset(as: .between(since: "2025-01-01", until: "2025-12-31")))
        #expect(!one.samePreset(as: .everything))
        #expect(!StyleLearnRange.everything.samePreset(as: nil))
        #expect(StyleLearnRange.lastTwelveMonths.samePreset(as: .lastTwelveMonths))
        #expect(!StyleLearnRange.lastTwelveMonths.samePreset(as: .everything))
    }

    @Test func aRangeSaysWhatNextFetchesOnlyWhenItFetches() {
        #expect(WritingStyleWords.fetch(.nothing) == nil)
        #expect(WritingStyleWords.fetch(.older) == L10n.learn_range_fetch_older())
        #expect(WritingStyleWords.fetch(.wholeHistory) == L10n.learn_range_fetch_all())
    }

    @Test(arguments: [UInt32(0), 1, 3])
    func theReviewSaysHowManyMessagesItCouldNotLoad(count: UInt32) {
        let report = StyleLearningReport(
            found: 12, usable: 8, languages: [], otherLanguages: 0, oldest: nil, newest: nil,
            notLoaded: count)
        let expected = count == 0 ? nil : L10n.learn_not_loaded(count: Int(count))
        #expect(WritingStyleWords.notLoaded(.ready(report: report)) == expected)
        #expect(WritingStyleWords.notLoaded(.nothingUsable(report: report)) == expected)
        if count == 1 { #expect(expected?.contains("1 message ") == true, "the singular is worded") }
        #expect(WritingStyleWords.notLoaded(.reading) == nil)
        #expect(WritingStyleWords.notLoaded(.fetching) == nil)
    }

    @Test func theSpokenLetterJoinsItsPartsWithOneComma() {
        // A greeting and a sign-off carry their own comma, and the parts are joined by one.
        func span(_ text: String) -> [WritingStyleSpan] {
            [WritingStyleSpan(text: text, placeholder: false)]
        }
        let letter = WritingStyleLetter(
            opening: span("Hi Bob,"), paragraphs: [], closing: span("Best regards, "),
            signsAs: "Alice", words: 120, paragraphCount: 3, shape: "")
        let spoken = RevealLetterSection.spoken(letter)
        #expect(!spoken.contains(",,"))
        #expect(spoken.hasPrefix(L10n.a11y_letter_opening(greeting: "Hi Bob") + ", "))
        #expect(spoken.contains(L10n.a11y_letter_closing(sign_off: "Best regards") + ", "))
    }

    @Test(arguments: ["Dear Bob,", "Dear Bob;", "Dear Bob:", "Dear Bob.", "Dear Bob"])
    func aGreetingIsSpokenWithoutThePunctuationItEndsOn(greeting: String) {
        let letter = WritingStyleLetter(
            opening: [WritingStyleSpan(text: greeting, placeholder: false)], paragraphs: [],
            closing: [], signsAs: "", words: 0, paragraphCount: 0, shape: "")
        #expect(RevealLetterSection.spoken(letter) == L10n.a11y_letter_opening(greeting: "Dear Bob"))
    }

    @Test func onlyTheEndingPunctuationGoes() {
        // An abbreviation inside a sign-off keeps its full stops; only the one it ends on goes.
        let letter = WritingStyleLetter(
            opening: [], paragraphs: [],
            closing: [WritingStyleSpan(text: "Best, e.g. Al.", placeholder: false)], signsAs: "",
            words: 0, paragraphCount: 0, shape: "")
        #expect(RevealLetterSection.spoken(letter) == L10n.a11y_letter_closing(sign_off: "Best, e.g. Al"))
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
            WritingStyleWords.blocked(.cannotSend(
                failure: .nowhereToSend,
                remedy: AiRemedyControl(remedy: .openAiEndpoint, control: .free())))
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
