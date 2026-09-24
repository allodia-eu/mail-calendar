// What Print hands the dialog (docs/reading-actions.md, "Printing a message"): nothing until the
// body has arrived, and then the header the reading view draws above that body.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct MessagePrintTests {
    private let avatar = Avatar(
        initials: "BT",
        light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        imagePath: nil
    )

    private var message: OpenedMessage {
        OpenedMessage(
            account: "a1", key: "m1", subject: "Quarterly planning", from: "Bob Tester",
            avatar: avatar, date: "2026-09-23 13:37"
        )
    }

    private func snapshot(pending: Bool = false, loadError: Bool = false) -> ReadingSnapshot {
        ReadingSnapshot(
            key: "m1",
            from: "Bob Tester <bob@test.local>",
            avatar: avatar,
            to: "Alice <alice@test.local>",
            cc: "",
            bcc: "",
            html: pending ? nil : "<p>Body</p>",
            plain: nil,
            hasRemoteImages: false,
            loadError: loadError,
            attachments: [],
            invitation: nil,
            pending: pending
        )
    }

    @Test func nothingToPrintUntilTheBodyHasArrived() {
        #expect(messagePrintDocument(message, nil, loadRemoteImages: false) == nil)
        #expect(messagePrintDocument(message, snapshot(pending: true), loadRemoteImages: false) == nil)
        #expect(messagePrintDocument(message, snapshot(loadError: true), loadRemoteImages: false) == nil)
    }

    @Test func thePageCarriesTheHeaderAboveTheBody() throws {
        let page = try #require(messagePrintDocument(message, snapshot(), loadRemoteImages: false))
        let from = try #require(page.range(of: "Bob Tester &lt;bob@test.local&gt;"))
        let sent = try #require(page.range(of: "2026-09-23 13:37"))
        let body = try #require(page.range(of: "<p>Body</p>"))
        #expect(page.contains("Quarterly planning"))
        #expect(from.lowerBound < sent.lowerBound && sent.lowerBound < body.lowerBound)
        // Cc and Bcc are empty on this message, so they are not on the page.
        #expect(!page.contains("\(L10n.compose_cc()):"))
    }
}
