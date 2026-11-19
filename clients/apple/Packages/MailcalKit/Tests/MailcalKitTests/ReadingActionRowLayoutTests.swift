// The action row above an open message, as it is drawn on a phone and in an iPad's reading pane
// (`docs/reading-actions.md`, `docs/reading-dark-mode.md`).
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

    /// The two widths the row is drawn at: a phone's, compact and icon-only, and an upright iPad's
    /// reading pane beside the list, regular, where the row keeps its labels while they fit.
    nonisolated static let widths: [Width] = [
        Width(compact: true, size: CGSize(width: 393, height: 852)),
        Width(compact: false, size: CGSize(width: 448, height: 1_100)),
    ]

    struct Width: CustomTestStringConvertible, Sendable {
        let compact: Bool
        let size: CGSize
        var testDescription: String { compact ? "phone" : "iPad reading pane" }
    }

    /// The reading view of a plain message at `width`, in `scheme`.
    private func screen(
        _ scheme: ColorScheme, _ width: Width, model: MailboxModel = MailboxModel()
    ) -> HostedScreen {
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
            size: width.size, scheme: scheme, compact: width.compact
        )
    }

    /// Which of archive and delete comes first is each platform's own, so the assertion is on the
    /// three groups and on the overflow being last, exactly as the contract states it.
    @Test(arguments: widths)
    func theRowOpensWithTheRepliesAndEndsWithTheOverflow(_ width: Width) throws {
        let expected = ["Reply", "Reply all", "Forward", "Archive", "Delete", "More actions"]
        let screen = screen(.light, width)
        let order = screen.leftToRight(expected)

        try #require(
            order.count == expected.count, "the action row drew \(order) of:\n\(screen.dump())"
        )
        #expect(Array(order.prefix(3)) == ["Reply", "Reply all", "Forward"])
        #expect(Set(order[3...4]) == ["Archive", "Delete"])
        #expect(order.last == "More actions")
    }

    /// A light app has nothing to switch, so the row draws no button for it.
    @Test(arguments: widths)
    func aLightAppDrawsNoSunOrMoon(_ width: Width) throws {
        let screen = screen(.light, width)
        try #require(!screen.elements("Reply").isEmpty, "no action row among:\n\(screen.dump())")
        #expect(screen.elements(toLight).isEmpty)
        #expect(screen.elements(toDark).isEmpty)
    }

    /// In a dark app the button opens the trailing group, every button of the row is wholly on the
    /// screen and a finger wide, and a press turns the sun into the moon.
    ///
    /// The preference is the core's, so the model holds a demo core: without one a press changes
    /// nothing. The app reads preferences back on the `Surface::Settings` signal; this core's
    /// signal goes nowhere, so the test reads them back itself.
    @Test(arguments: widths)
    func aDarkAppDrawsTheButtonBeforeArchiveAndKeepsTheRowOnScreen(_ width: Width) throws {
        let model = MailboxModel()
        let app = MailcalApp.newDemo(
            observer: Unobserved(), logger: Unobserved(), logLevel: .error,
            deviceTimezone: "Europe/Amsterdam"
        )
        model.app = app
        model.setDarkMessageBackground(true)
        model.reloadPreferences(app, repaint: false)
        let screen = screen(.dark, width, model: model)
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
        model.reloadPreferences(app, repaint: false)
        screen.settle()
        #expect(!screen.elements(toDark).isEmpty, "pressing the sun did not bring the moon")
    }
}

/// A demo core's observer and logger, for a test that reads the core back itself.
private final class Unobserved: Observer, MailcalBindings.Logger {
    func surfaceChanged(surface _: Surface) {}
    func log(level _: LogLevel, target _: String, message _: String) {}
}
#endif
