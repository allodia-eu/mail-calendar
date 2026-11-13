import CoreGraphics
import Testing

@testable import MailcalUI

/// A block at the top of a scrolled page that grows or shrinks out of sight moves the page with
/// it, so what the person was looking at stays where it was; one they can see moves nothing.
@Suite struct ScrolledPastTests {
    @Test func aNoticeRaisedAboveTheVisibleTopKeepsThePageWhereItWas() {
        let before = CGRect(x: 0, y: -300, width: 460, height: 40)
        let after = CGRect(x: 0, y: -300, width: 460, height: 140)
        #expect(ScrolledPast.shift(from: before, to: after) == 100)
    }

    @Test func aNoticeClearedAboveTheVisibleTopKeepsThePageWhereItWas() {
        let before = CGRect(x: 0, y: -300, width: 460, height: 140)
        let after = CGRect(x: 0, y: -300, width: 460, height: 40)
        #expect(ScrolledPast.shift(from: before, to: after) == -100)
    }

    @Test func aBlockEndingExactlyAtTheVisibleTopCountsAsOutOfSight() {
        let before = CGRect(x: 0, y: -40, width: 460, height: 40)
        let after = CGRect(x: 0, y: -40, width: 460, height: 140)
        #expect(ScrolledPast.shift(from: before, to: after) == 100)
    }

    @Test func aNoticeRaisedWhereThePersonCanSeeItMovesNothing() {
        let before = CGRect(x: 0, y: -20, width: 460, height: 40)
        let after = CGRect(x: 0, y: -20, width: 460, height: 140)
        #expect(ScrolledPast.shift(from: before, to: after) == nil)
    }

    @Test func scrollingAloneMovesNothing() {
        let before = CGRect(x: 0, y: -300, width: 460, height: 40)
        let after = CGRect(x: 0, y: -340, width: 460, height: 40)
        #expect(ScrolledPast.shift(from: before, to: after) == nil)
    }
}
