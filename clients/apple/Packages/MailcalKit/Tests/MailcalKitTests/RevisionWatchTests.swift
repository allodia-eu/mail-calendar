// When the composer counts the message as changed, for the autosave (`docs/drafts.md`).

import Testing

@testable import MailcalUI

@Suite struct RevisionWatchTests {
    /// What the watch answers for each read in turn.
    private func answers(_ reads: [Int?]) -> [Bool] {
        var watch = RevisionWatch()
        return reads.map { watch.saw($0) }
    }

    @Test func aPageThatAnsweredLateIsNotAnEdit() {
        // The first read comes before the page has loaded; the next one answers with the count
        // the seeds left. Nobody has typed, so nothing is saved.
        #expect(answers([nil, 7, 7]) == [false, false, false])
    }

    @Test func aCountThatMovesIsAnEdit() {
        #expect(answers([3, 4, 4]) == [false, true, false])
    }

    @Test func aReadThatFailsLaterChangesNothing() {
        #expect(answers([3, nil, 3, 5]) == [false, false, false, true])
    }
}
