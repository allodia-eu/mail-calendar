// The order the learn sheet walks (docs/ai.md, "Learning"): an account only when there is a choice,
// a range, the report the device reads, the consent, then the run. Each rule here is one a sheet
// can break while looking right: offering Learn over a report that found nothing usable, letting an
// abandoned range's report overwrite the current one, or ending "up to a date" at the start of the
// day the person chose rather than at its end.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct LearnWritingStyleFlowTests {
    @Test func theAccountIsAskedOnlyWhenThereIsAChoice() {
        #expect(!LearnWritingStyleFlow.asksForAccount(["alice"]))
        #expect(LearnWritingStyleFlow.asksForAccount(["alice", "bob"]))
        #expect(LearnWritingStyleFlow(accounts: ["alice", "bob"]).account == "alice")
    }

    /// The core's `until` is inclusive, so the chosen day ends one second before the next begins,
    /// in the person's own zone.
    @Test func upToADateIncludesTheWholeOfThatDay() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try #require(TimeZone(identifier: "Europe/Amsterdam"))
        let noon = try #require(calendar.date(from: DateComponents(year: 2024, month: 12, day: 31, hour: 12)))
        let bounds = LearnRangeChoice.until(noon).bounds(calendar: calendar)
        #expect(bounds.since == nil)
        #expect(bounds.until == 1_735_685_999)
        let everything = LearnRangeChoice.everything.bounds(calendar: calendar)
        #expect(everything.since == nil && everything.until == nil)
    }

    @Test func aReportForAnAbandonedChoiceIsDropped() {
        var flow = LearnWritingStyleFlow(accounts: ["alice"])
        let first = flow.readReport()
        let second = flow.readReport()
        flow.reportArrived(.success(report(usable: 1)), generation: first)
        #expect(flow.report == .reading)
        flow.reportArrived(.success(report(usable: 7)), generation: second)
        #expect(flow.report == .ready(report(usable: 7)))
    }

    @Test func nothingUsableOffersNoLearnButton() {
        var flow = LearnWritingStyleFlow(accounts: ["alice"])
        flow.reportArrived(.success(report(usable: 0)), generation: flow.readReport())
        #expect(!flow.canLearn)
        let started = flow.consent()
        #expect(!started)
        #expect(flow.phase == .choosing)
    }

    @Test func aReportThatFailedOffersNoLearnButton() {
        var flow = LearnWritingStyleFlow(accounts: ["alice"])
        flow.reportArrived(.failure(.NoSentFolder), generation: flow.readReport())
        #expect(flow.report == .failed(.NoSentFolder))
        #expect(!flow.canLearn)
    }

    @Test func consentStartsTheRunAndItsStyleOpensAfterwards() {
        var flow = LearnWritingStyleFlow(accounts: ["alice"])
        flow.reportArrived(.success(report(usable: 12)), generation: flow.readReport())
        #expect(flow.canLearn)
        let started = flow.consent()
        #expect(started)
        #expect(flow.phase == .learning)
        // Stop is the only way out of a run that is going.
        #expect(!flow.dismissible)
        flow.learningFinished(.success(LearnReport(styleId: "s1", languages: ["en"], messages: 12, charge: nil)))
        #expect(flow.phase == .learned(styleId: "s1"))
    }

    @Test func aStoppedRunSaysNoStyleWasSaved() {
        var flow = LearnWritingStyleFlow(accounts: ["alice"])
        flow.reportArrived(.success(report(usable: 12)), generation: flow.readReport())
        _ = flow.consent()
        flow.learningFinished(.failure(.Cancelled))
        #expect(flow.phase == .failed(.Cancelled))
        #expect(flow.dismissible)
        #expect(writingStyleFailureText(.Cancelled, route: .ownEndpoint) == L10n.ai_error_cancelled())
    }

    private func report(usable: UInt32) -> CorpusReport {
        CorpusReport(
            found: 20, usable: usable, undetected: 0, languages: [], oldest: nil, newest: nil,
            horizon: nil
        )
    }
}
