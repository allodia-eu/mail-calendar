// The archive/delete auto-advance rule: next one down, else the one above, else nothing.
//
// Pinning it here rather than through the UI is the point of keeping `messageAfterRemoving` pure:
// the end-of-list case in particular is the one a hand test skips, because it only shows up on the
// last message of a folder.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct AutoAdvanceTests {
    private func stop(_ key: String, account: String = "a0") -> OpenedMessage {
        OpenedMessage(
            account: account,
            key: key,
            subject: "s-\(key)",
            from: "f-\(key)",
            // Any avatar: what is under test is which message comes next, not what it draws.
            avatar: Avatar(
                initials: "F",
                light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
                dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
                imagePath: nil
            ),
            date: "d"
        )
    }

    @Test func advancesToTheNextMessageDown() {
        let stops = [stop("1"), stop("2"), stop("3")]
        #expect(messageAfterRemoving(stop("2"), from: stops)?.key == "3")
    }

    @Test func advancesFromTheFirstMessage() {
        let stops = [stop("1"), stop("2"), stop("3")]
        #expect(messageAfterRemoving(stop("1"), from: stops)?.key == "2")
    }

    /// The end of the list falls *back* rather than emptying the pane, clearing out the bottom of a
    /// folder should not strand the reader on a placeholder with a full mailbox beside it.
    @Test func theLastMessageFallsBackToTheOneAbove() {
        let stops = [stop("1"), stop("2"), stop("3")]
        #expect(messageAfterRemoving(stop("3"), from: stops)?.key == "2")
    }

    @Test func theOnlyMessageLeavesNothingToOpen() {
        #expect(messageAfterRemoving(stop("1"), from: [stop("1")]) == nil)
    }

    @Test func anEmptyListLeavesNothingToOpen() {
        #expect(messageAfterRemoving(stop("1"), from: []) == nil)
    }

    /// A message that is no longer on screen has no neighbours to speak of, so the pane just empties
    /// the behaviour before auto-advance existed.
    @Test func aMessageNotInTheListLeavesNothingToOpen() {
        #expect(messageAfterRemoving(stop("9"), from: [stop("1"), stop("2")]) == nil)
    }

    /// A provider key is unique only *within* an account, so two accounts can mint the same one. The
    /// match has to be on both halves or the unified inbox advances into the wrong account.
    @Test func theSameKeyInAnotherAccountIsADifferentMessage() {
        let stops = [stop("1", account: "a0"), stop("1", account: "a1"), stop("2", account: "a1")]
        let next = messageAfterRemoving(stop("1", account: "a1"), from: stops)
        #expect(next?.account == "a1")
        #expect(next?.key == "2")
    }

    @Test func matchingIsNotConfusedByAKeyThatOnlyMatchesTheAccount() {
        let stops = [stop("1", account: "a0"), stop("2", account: "a0")]
        #expect(messageAfterRemoving(stop("3", account: "a0"), from: stops) == nil)
    }
}
