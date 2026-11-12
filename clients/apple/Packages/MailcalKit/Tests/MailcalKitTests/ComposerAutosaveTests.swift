// The composer's idle save counts an edit only against the editor as its seeds left it
// (docs/drafts.md, "When a save happens").

import Testing

@testable import MailcalUI

@Suite struct ComposerAutosaveTests {

    @Test func nothingCountsWhileTheSeedsAreGoingIn() {
        var watch = EditorRevisionWatch()
        #expect(!watch.changed(now: 0, seeded: nil))
        #expect(!watch.changed(now: 4, seeded: nil))
    }

    @Test func theSeedsAreTheBaselineNotAnEdit() {
        // The quote, the signature and a reopened quote's attribution moved the count to 4 before
        // the first sample: an untouched composer must not read as written.
        var watch = EditorRevisionWatch()
        #expect(!watch.changed(now: 0, seeded: nil))
        #expect(!watch.changed(now: 4, seeded: 4))
        #expect(!watch.changed(now: 4, seeded: 4))
    }

    @Test func typingBeforeTheFirstSampleIsAnEdit() {
        // The seeds left 4 and the user typed before the first sample: that sample is the edit,
        // not the baseline, so a short reply written straight after opening is saved.
        var watch = EditorRevisionWatch()
        #expect(watch.changed(now: 7, seeded: 4))
        #expect(!watch.changed(now: 7, seeded: 4))
        #expect(watch.changed(now: 9, seeded: 4))
    }

    @Test func aCountThatCouldNotBeReadChangesNothing() {
        var watch = EditorRevisionWatch()
        #expect(!watch.changed(now: -1, seeded: 4))
        #expect(!watch.changed(now: 4, seeded: 4))
        #expect(!watch.changed(now: -1, seeded: 4))
        #expect(watch.changed(now: 5, seeded: 4))
    }

    @Test func aBaselineThatCouldNotBeReadIsTakenFromTheFirstSample() {
        var watch = EditorRevisionWatch()
        #expect(!watch.changed(now: 6, seeded: -1))
        #expect(watch.changed(now: 8, seeded: -1))
    }
}
