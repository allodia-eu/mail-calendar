// The From field fits the column it is in. A pop-up button's own width is its longest item's, and
// a sender's name is the person's to choose, so held at that width one long account made the
// composer wider than its pane, and the pane's scroll view cut it off on both sides.
//
// SwiftUI draws the button itself, with no AppKit control behind it to measure, so it is read
// through the test agent (docs/test-agent.md), as the running app's is.

#if os(macOS) && DEBUG
import AppKit
import Foundation
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@MainActor
@Suite(.serialized)
struct FromAccountFieldWidthTests {

    private let accounts = [
        AccountRow(id: "short", email: "me@example.test", name: "Me", expanded: true),
        AccountRow(
            id: "long",
            email: "someone.with.a.long.address@a-long-domain-name.example.test",
            name: "A Sender Whose Name Is Much Longer Than Any Column",
            expanded: true
        ),
    ]

    @Test func aLongSenderFitsANarrowColumn() async throws {
        let bounds = try await popUpBounds(width: 300, selection: "long")
        #expect(bounds.minX >= 0)
        #expect(bounds.maxX <= 300)
    }

    @Test func aShortSenderKeepsItsOwnWidthInAWideColumn() async throws {
        // Fitting the column is not filling it: with room to spare the button is as wide as the
        // longest sender and no wider.
        let wide = try await popUpBounds(width: 1600, selection: "short")
        let wider = try await popUpBounds(width: 2400, selection: "short")
        #expect(wide.width > 0)
        #expect(wide.width == wider.width)
    }

    /// The field alone in a column `width` wide, and its pop-up's bounds in the window.
    private func popUpBounds(width: CGFloat, selection: String) async throws -> CGRect {
        let title = "From field \(Int(width))"
        let frame = NSRect(x: 0, y: 0, width: width, height: 100)
        let field = FromAccountField(accounts: accounts, selection: .constant(selection))
            .frame(width: width, alignment: .leading)
        let window = NSWindow(contentRect: frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.title = title
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: field)
        window.orderFront(nil)
        window.layoutIfNeeded()
        defer { window.close() }

        let reply = await ask(["verb": "query", "window": title, "role": "AXPopUpButton"])
        let elements = reply["elements"] as? [[String: Any]] ?? []
        #expect(elements.count == 1, "\(reply)")
        let bounds = try #require(elements.first?["bounds"] as? [Int])
        return CGRect(x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3])
    }

    private func ask(_ request: [String: Any]) async -> [String: Any] {
        let line = String(decoding: try! JSONSerialization.data(withJSONObject: request), as: UTF8.self)
        let reply = await AgentProtocol.handle(line)
        return try! JSONSerialization.jsonObject(with: Data(reply.utf8)) as! [String: Any]
    }
}
#endif
