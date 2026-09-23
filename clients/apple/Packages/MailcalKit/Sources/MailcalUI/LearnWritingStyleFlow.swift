// Where the "Learn my writing style" sheet stands (docs/ai.md, "Learning"). A value type with no
// view in it, so the suite drives the order the contract fixes: an account (asked only when there
// is a choice), a range, the report read on the device, the consent on the same sheet, then the
// run. The blocking calls stay in the view; this decides what their answers mean.

import Foundation
import MailcalBindings

/// Which sent mail to learn from.
enum LearnRangeChoice: Equatable {
    /// Everything the device holds.
    case everything
    /// Everything up to and including this day, in `calendar`'s zone.
    case until(Date)

    /// The `(since, until)` pair the core takes, in seconds since the Unix epoch. The core's
    /// `until` is inclusive, so the chosen day ends one second before the next one starts.
    func bounds(calendar: Calendar) -> (since: Int64?, until: Int64?) {
        switch self {
        case .everything:
            return (nil, nil)
        case let .until(day):
            let start = calendar.startOfDay(for: day)
            let next = calendar.date(byAdding: .day, value: 1, to: start) ?? start
            return (nil, Int64(next.timeIntervalSince1970) - 1)
        }
    }
}

struct LearnWritingStyleFlow: Equatable {
    /// What the device found for the current choice.
    enum Report: Equatable {
        case reading
        case ready(CorpusReport)
        case failed(WritingStyleFailure)
    }

    /// Where the sheet stands.
    enum Phase: Equatable {
        /// Choosing an account and a range, with the report for that choice beneath.
        case choosing
        /// Consent given; the run is going.
        case learning
        /// The run stopped without a style.
        case failed(WritingStyleFailure)
        /// A style was learned; the sheet hands over to the reveal.
        case learned(styleId: String)
    }

    var account: String?
    var range = LearnRangeChoice.everything
    private(set) var report = Report.reading
    private(set) var phase = Phase.choosing
    /// Which request the report on screen belongs to. The report is read again whenever the
    /// choice changes, and an answer for a choice already abandoned must not replace a later one.
    private(set) var generation = 0

    /// Opens on the first account, the only one when there is no choice to make.
    init(accounts: [String]) {
        account = accounts.first
    }

    /// Whether the sheet asks which account: only when there is more than one.
    static func asksForAccount(_ accounts: [String]) -> Bool {
        accounts.count > 1
    }

    /// Starts reading the report for the current choice; hand the result back with the number
    /// this returns.
    mutating func readReport() -> Int {
        generation += 1
        report = .reading
        return generation
    }

    /// The report for request `generation` arrived. An answer to an earlier request is dropped.
    mutating func reportArrived(_ result: Result<CorpusReport, WritingStyleFailure>, generation: Int) {
        guard generation == self.generation, phase == .choosing else { return }
        switch result {
        case let .success(report): self.report = .ready(report)
        case let .failure(failure): report = .failed(failure)
        }
    }

    /// Whether there is anything to learn from, which is what puts the consent and the Learn
    /// button on screen. Nothing usable says so and offers no button.
    var canLearn: Bool {
        guard phase == .choosing, account != nil, case let .ready(report) = report else {
            return false
        }
        return report.usable > 0
    }

    /// The person pressed Learn on the consent. Returns whether the run should start.
    mutating func consent() -> Bool {
        guard canLearn else { return false }
        phase = .learning
        return true
    }

    /// The run ended.
    mutating func learningFinished(_ result: Result<LearnReport, WritingStyleFailure>) {
        guard phase == .learning else { return }
        switch result {
        case let .success(report): phase = .learned(styleId: report.styleId)
        case let .failure(failure): phase = .failed(failure)
        }
    }

    /// Whether the sheet may be put away by the person: not while a run is going, whose only way
    /// out is Stop.
    var dismissible: Bool {
        phase != .learning
    }
}
