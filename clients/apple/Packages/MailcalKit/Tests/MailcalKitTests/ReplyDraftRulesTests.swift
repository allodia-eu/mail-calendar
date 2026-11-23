// What a composer does with Draft a reply's record that is not drawing (ReplyDraftRules.swift) and
// how the editor's channel hands its messages on (ComposerHostChannel.swift): every message
// reaches the core's report, the reply-draft notice included, which the parse leaves out. What the
// record says is the core's, tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ReplyDraftRulesTests {
    @Test func tryAgainOpensTheInputAndEveryOtherRemedyOpensElsewhere() {
        #expect(ComposerRemedyRoute(.tryAgain) == .input)
        for remedy: AiRemedy in [.signInToAllodia, .openAiEndpoint, .learnStyle, .chooseStyle] {
            #expect(ComposerRemedyRoute(remedy) == .elsewhere)
        }
    }

    @Test func aRemedyOpensSettingsWhereItsFixIs() {
        #expect(settingsCategory(for: .openAiEndpoint) == .advanced)
        #expect(settingsCategory(for: .learnStyle) == .writingStyle)
        #expect(settingsCategory(for: .chooseStyle) == .writingStyle)
        #expect(settingsCategory(for: .signInToAllodia) == nil, "the sign-in opens over the composer")
        #expect(settingsCategory(for: .tryAgain) == nil)
    }

    @Test func learnStyleOpensTheLearningFlowAndNoOtherRemedyDoes() {
        #expect(opensLearningFlow(.learnStyle))
        #expect(settingsCategory(for: .learnStyle) == .writingStyle, "which draws the flow's host")
        let others: [AiRemedy] = [
            .chooseStyle, .chooseAccount, .chooseRange, .openAiEndpoint, .signInToAllodia, .tryAgain,
        ]
        for remedy in others {
            #expect(!opensLearningFlow(remedy))
        }
    }

    @Test func eachSuggestionHasItsOwnWords() {
        let all: [ReplyDraftSuggestion] = [.yes, .no, .moreInformation, .holding]
        #expect(Set(all.map(ReplyDraftWords.suggestion)).count == all.count)
    }

    @Test func aGapIsNamedAsTheReplyCarriesItAndATaskAsWritten() {
        let gap = ReplyDraftItem(index: 0, kind: .fillIn, text: "[date]", done: false, tick: nil)
        #expect(ReplyDraftWords.item(gap) == L10n.composer_task_fill_in(placeholder: "[date]"))
        let task = ReplyDraftItem(
            index: 1, kind: .do, text: "Book the room", done: false, tick: .free())
        #expect(ReplyDraftWords.item(task) == "Book the room")
    }

    @Test func sendsQuestionNamesHowManyAreOpen() {
        #expect(
            ReplyDraftWords.sendQuestion(ReplyDraftSendQuestion(open: 2))
                == L10n.composer_send_open_message(count: 2))
    }
}

@Suite @MainActor struct ComposerHostChannelTests {
    @Test func everyMessageReachesTheReportTheReplyDraftNoticeIncluded() {
        let channel = ComposerHostChannel()
        var handed: [String] = []
        channel.onMessage = { handed.append($0) }
        var carets = 0
        channel.onCaret = { _ in carets += 1 }
        let notice = #"{"id":0,"request":{"reply_draft":{"serial":1,"lead":"draft","inserted":true,"left":[],"edits":0}}}"#
        let caret = #"{"id":0,"request":{"caret":{"top":1,"bottom":2}}}"#
        channel.receive(notice)
        channel.receive(caret)
        channel.receive("not a message")
        #expect(handed == [notice, caret, "not a message"])
        #expect(carets == 1, "a request the parse knows still reaches its handler")
    }
}
