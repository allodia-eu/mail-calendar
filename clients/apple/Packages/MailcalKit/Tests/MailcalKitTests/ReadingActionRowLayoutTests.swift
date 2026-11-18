// The action row above an open message on a phone, as it is drawn (`docs/reading-actions.md`,
// `docs/reading-dark-mode.md`).
//
// Facts about the screen: the row's order; the sun/moon button only in a dark app, opening the
// trailing group; and a seventh button still leaving the row inside the screen, because a row wider
// than the screen pushes the whole reading view past both edges of an iPhone. Hosted in a window of
// its own (`HostedScreen`), so none of it needs the app launched.

#if os(iOS) && DEBUG
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite @MainActor struct ReadingActionRowLayoutTests {
    private let toLight = "Show messages with a light background"
    private let toDark = "Show messages with a dark background"

    private static let avatar = Avatar(
        initials: "BT",
        light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        imagePath: nil
    )

    /// The reading view of a plain message, on a phone, in `scheme`.
    private func screen(_ scheme: ColorScheme, model: MailboxModel = MailboxModel()) -> HostedScreen {
        model.reading = ReadingSnapshot(
            key: "m1", from: "Bob Tester <bob@example.test>", avatar: Self.avatar,
            to: "Alice <alice@example.test>", cc: "", bcc: "", html: nil, plain: "Body.",
            hasRemoteImages: false, loadError: false, attachments: [], invitation: nil,
            pending: false
        )
        let message = OpenedMessage(
            account: "acct-1", key: "m1", subject: "Quarterly planning",
            from: "Bob Tester <bob@example.test>", avatar: Self.avatar, date: "9 November 2026"
        )
        return HostedScreen(
            ReadingView(
                model: model, message: message,
                onReply: {}, onReplyAll: {}, onForward: {}, onArchive: {}, onDelete: {}
            ),
            scheme: scheme
        )
    }

    /// Which of archive and delete comes first is each platform's own, so the assertion is on the
    /// three groups and on the overflow being last, exactly as the contract states it.
    @Test func theRowOpensWithTheRepliesAndEndsWithTheOverflow() throws {
        let expected = ["Reply", "Reply all", "Forward", "Archive", "Delete", "More actions"]
        let screen = screen(.light)
        let order = screen.leftToRight(expected)

        try #require(
            order.count == expected.count, "the action row drew \(order) of:\n\(screen.dump())"
        )
        #expect(Array(order.prefix(3)) == ["Reply", "Reply all", "Forward"])
        #expect(Set(order[3...4]) == ["Archive", "Delete"])
        #expect(order.last == "More actions")
    }

    /// A light app has nothing to switch, so the row draws no button for it.
    @Test func aLightAppDrawsNoSunOrMoon() {
        let screen = screen(.light)
        #expect(screen.elements(toLight).isEmpty)
        #expect(screen.elements(toDark).isEmpty)
    }

    /// In a dark app the button opens the trailing group, every button of the row is wholly on the
    /// screen and a finger wide, and a press turns the sun into the moon.
    @Test func aDarkAppDrawsTheButtonBeforeArchiveAndKeepsTheRowOnScreen() throws {
        let model = MailboxModel()
        model.setDarkMessageBackground(true)
        let screen = screen(.dark, model: model)
        let row = ["Reply", "Reply all", "Forward", toLight, "Archive", "Delete", "More actions"]
        let order = screen.leftToRight(row)
        try #require(order.count == row.count, "the action row drew \(order) of:\n\(screen.dump())")
        #expect(order[3] == toLight, "the button does not open the trailing group: \(order)")

        let width = screen.window.bounds.width
        for label in row {
            let frame = try #require(screen.elements(label).first?.bounds, "\(label) has no frame")
            #expect(frame.minX >= 0 && frame.maxX <= width, "\(label) runs off the screen: \(frame)")
            #expect(frame.width >= 44, "\(label) is narrower than a finger: \(frame)")
        }

        let sun = try #require(screen.elements(toLight).first)
        _ = try AgentPlatform.activate(sun.object)
        screen.settle()
        #expect(!screen.elements(toDark).isEmpty, "pressing the sun did not bring the moon")
    }
}
#endif
