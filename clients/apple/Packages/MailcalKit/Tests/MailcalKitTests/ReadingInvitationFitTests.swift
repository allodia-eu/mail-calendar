// The invitation card in the Mac reading pane: drawn at its own height while the pane has room,
// and scrolled inside itself once it would crowd the pane.
//
// The Mac's reading column does not scroll as a whole (the body's web view scrolls itself), so a
// card at its full height in a short pane makes the column taller than the pane. The window holds
// still regardless (`macPaneHeight`), but the bottom of the card, Accept, Maybe and Decline
// included, is then cut off with nothing to scroll it into view.

#if os(macOS)
import AppKit
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite @MainActor struct ReadingInvitationFitTests {
    private static let avatar = Avatar(
        initials: "BT",
        light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        imagePath: nil
    )

    private let opened = OpenedMessage(
        account: "acct-1", key: "m1", subject: "Quarterly planning",
        from: "Bob Tester <bob@example.test>", avatar: avatar, date: "9 November 2026"
    )

    /// An invitation with every section the card draws, which with the reading header above it is
    /// taller than a minimum-size window.
    private var tallInvitation: InvitationCard {
        InvitationCard(
            kind: .rsvp,
            organizer: "Bob Tester <bob@example.test>",
            summary: "Quarterly planning",
            location: "Room 4",
            description: (1...60).map { "Agenda item \($0)." }.joined(separator: "\n"),
            descriptionTruncated: false,
            startsAt: "2026-11-09T09:30:00Z",
            endsAt: "2026-11-09T10:30:00Z",
            allDay: false,
            recurring: false,
            myResponse: .needsAction,
            attendees: AttendeeTally(total: 3, accepted: 2, declined: 0, tentative: 0, needsAction: 1),
            conflictCount: 0,
            conflictsKnown: true,
            preview: InvitationPreview(
                days: [], timed: [], allDay: [], allDayLanes: 0, timezone: "Europe/Amsterdam"
            ),
            canRespond: true,
            canComment: true,
            canChooseNotify: true
        )
    }

    private func snapshot(invitation: InvitationCard?) -> ReadingSnapshot {
        ReadingSnapshot(
            key: "m1", from: "Bob Tester <bob@example.test>", avatar: Self.avatar,
            to: "Alice <alice@example.test>", cc: "", bcc: "", html: nil, plain: "Body.",
            hasRemoteImages: false, loadError: false, attachments: [], invitation: invitation,
            pending: false
        )
    }

    /// The reading view, hosted in a window of `height` so its layout runs and the card measures
    /// itself, as it does on screen.
    private func hostedReadingView(invitation: InvitationCard?, height: CGFloat)
        -> NSHostingController<some View>
    {
        let model = MailboxModel()
        model.reading = snapshot(invitation: invitation)
        let controller = NSHostingController(
            rootView: ReadingView(
                model: model, message: opened,
                onReply: {}, onReplyAll: {}, onForward: {}, onArchive: {}, onDelete: {}
            )
        )
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 640, height: height),
            styleMask: [.titled], backing: .buffered, defer: false
        )
        window.contentViewController = controller
        window.setContentSize(NSSize(width: 640, height: height))
        // The card's height is measured in one layout pass and applied in the next.
        for _ in 0..<5 {
            controller.view.layoutSubtreeIfNeeded()
            RunLoop.main.run(until: Date().addingTimeInterval(0.05))
        }
        return controller
    }

    /// The height a view asks for when nothing constrains it.
    private func idealHeight(of controller: NSHostingController<some View>) -> CGFloat {
        controller.sizingOptions = .intrinsicContentSize
        return controller.view.intrinsicContentSize.height
    }

    @Test func aTallCardLeavesTheReadingViewWithinItsPane() {
        let controller = hostedReadingView(invitation: tallInvitation, height: 560)
        let answered = controller.sizeThatFits(in: CGSize(width: 640, height: 560)).height
        #expect(answered <= 560)
    }

    @Test func aCardWithRoomIsDrawnAtItsOwnHeight() {
        // A card that fits is not squeezed: in a pane with room, the card adds exactly what it
        // measures on its own, so the scroll view around it hugs it rather than taking a share.
        let card = idealHeight(of: NSHostingController(rootView: InvitationBanner(
            snapshot: snapshot(invitation: tallInvitation),
            zone: "Europe/Amsterdam", use24Hour: true,
            account: "acct-1", messageKey: "m1",
            writeStatus: .idle,
            respond: { _, _, _, _ in }
        )))
        let with = idealHeight(of: hostedReadingView(invitation: tallInvitation, height: 1_200))
        let without = idealHeight(of: hostedReadingView(invitation: nil, height: 1_200))
        #expect(card > 0)
        #expect(abs((with - without) - card) <= 1)
    }
}
#endif
