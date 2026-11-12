// The composer's idle save counts an edit only against the editor as its seeds left it
// (docs/drafts.md, "When a save happens").

import Testing

@testable import MailcalUI

@Suite struct ComposerAutosaveTests {

    /// Feeds one watch the samples in order, each a change count and the seeds' count, and answers
    /// what it made of each. `#expect` evaluates its argument in a closure, which cannot call a
    /// mutating method, so the samples are taken here and the answers asserted together.
    private func answers(_ samples: [(now: Int, seeded: Int?)]) -> [Bool] {
        var watch = EditorRevisionWatch()
        return samples.map { watch.changed(now: $0.now, seeded: $0.seeded) }
    }

    @Test func nothingCountsWhileTheSeedsAreGoingIn() {
        #expect(answers([(0, nil), (4, nil)]) == [false, false])
    }

    @Test func theSeedsAreTheBaselineNotAnEdit() {
        // The quote, the signature and a reopened quote's attribution moved the count to 4 before
        // the first sample: an untouched composer must not read as written.
        #expect(answers([(0, nil), (4, 4), (4, 4)]) == [false, false, false])
    }

    @Test func typingBeforeTheFirstSampleIsAnEdit() {
        // The seeds left 4 and the user typed before the first sample: that sample is the edit,
        // not the baseline, so a short reply written straight after opening is saved.
        #expect(answers([(7, 4), (7, 4), (9, 4)]) == [true, false, true])
    }

    @Test func aCountThatCouldNotBeReadChangesNothing() {
        #expect(answers([(-1, 4), (4, 4), (-1, 4), (5, 4)]) == [false, false, false, true])
    }

    @Test func aBaselineThatCouldNotBeReadIsTakenFromTheFirstSample() {
        #expect(answers([(6, -1), (8, -1)]) == [false, true])
    }
}
