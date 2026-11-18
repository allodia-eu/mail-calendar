// An account's page in Settings → Accounts (docs/accounts.md), driven against the linked harness
// accounts rather than the showcase: alice's mailbox used for mail alone, and alice's and bob's
// calendar and contacts as accounts of their own. The showcase has no account its page could
// suggest linking, and a change on its page answers nothing a person would see.
//
// So this is the one class here that needs the local harness (`scripts/dev/harness.sh up`) and the
// certificate `test-ui.sh` hands the runner. Without either it skips and says why.
//
// `stalwart-linked` connects its three canned accounts afresh on every launch, so the switch-off
// and the removal below are gone by the next run.

import Darwin
import XCTest

@MainActor
final class AccountPageTests: XCTestCase {
    private let addAccount = Filter.id("settings-add-account")

    func testUsesLinksAndRemovalOnAnAccountsPage() throws {
        let app = try launchOnTheLinkedHarness()
        try openAccounts(app)

        // Alice's mailbox: her calendar account is suggested for both slots, and Link takes it.
        try openPage(app, kind: "IMAP")
        let link = Filter.button("Link")
        // The page is a lazy list: a row is built only once it is scrolled to.
        try reveal(link, in: app)
        XCTAssertEqual(try app.waitAll(link).count, 2, "a suggestion under each slot")
        XCTAssertTrue(
            try app.exists(.text("Suggested: alice@test.local (CalDAV/CardDAV)")),
            "the suggestion is not alice's calendar account"
        )
        try app.activate(try reveal(link, in: app))
        // A filled slot is no longer a picker: the use's row says where it comes from.
        let from = "From alice@test.local (CalDAV/CardDAV)"
        XCTAssertTrue(
            try wait(until: {
                try app.query(.textLike(from).role("AXButton")).contains { $0.label.hasPrefix("Calendar") }
            }),
            "Link did not pick alice's calendar account"
        )
        XCTAssertTrue(
            try wait(until: { try app.query(link).count == 1 }),
            "the suggestion stayed after it was linked"
        )
        try back(app)

        // Alice's calendar account: switching Contacts off asks first.
        try openPage(app, kind: "CalDAV/CardDAV")
        let contacts = Filter.text("Contacts").role("AXSwitch")
        XCTAssertEqual(try reveal(contacts, in: app).active, true)
        let question = Filter.text("Switch off Contacts?")
        try switchOff(contacts, asking: question, in: app)
        try app.press(.button("Cancel"))
        try app.waitGone(question, timeout: ShowcaseApp.timeout)
        XCTAssertEqual(try app.wait(contacts).active, true, "Cancel switched Contacts off")

        try switchOff(contacts, asking: question, in: app)
        try app.press(.button("Switch off"))
        XCTAssertTrue(
            try wait(until: { try app.wait(contacts).active == false }), "Switch off left Contacts on"
        )
        // Calendar is now the account's last use, so it is held, and the page says why.
        let calendar = Filter.text("Calendar").role("AXSwitch")
        XCTAssertEqual(try app.wait(calendar).active, true)
        XCTAssertTrue(
            try wait(until: { try !app.wait(calendar).enabled }), "the last use can be switched off"
        )
        XCTAssertTrue(
            try app.appears(
                .text("An account is used for at least one of these. Remove it instead."), timeout: 5
            ),
            "the held switch does not say why"
        )

        // Removing it asks, names the mailbox that loses its link, and returns to the list.
        try app.activate(try reveal(.button("Remove account…"), in: app))
        try app.wait(.text("Remove account?"), "nothing was asked")
        XCTAssertTrue(
            try app.exists(.textLike("It is also unlinked from alice@test.local (IMAP).")),
            "the removal does not name the account that loses its link"
        )
        try app.press(.button("Remove"))
        try app.wait(addAccount, "the removal did not return to the list")
        XCTAssertTrue(
            try wait(until: { try self.row(app, kind: "CalDAV/CardDAV") == nil }),
            "the removed account is still listed"
        )
        XCTAssertNotNil(try row(app, kind: "IMAP"), "the mailbox went with it")
    }

    // MARK: Getting there

    private func launchOnTheLinkedHarness() throws -> App {
        guard let ca = ProcessInfo.processInfo.environment["MAILCAL_EXTRA_CA"] else {
            throw XCTSkip(
                "no harness certificate: run this through clients/apple/Scripts/test-ui.sh")
        }
        for port: UInt16 in [12993, 28080] where !Self.listening(on: port) {
            throw XCTSkip("the harness is not up on port \(port): scripts/dev/harness.sh up")
        }
        let app = try ShowcaseApp.launch(
            ["MAILCAL_DEV_ACCOUNT": "stalwart-linked", "MAILCAL_EXTRA_CA": ca], args: [])
        // A simulator that has never opened this store starts on the welcome screen, which is not
        // what this tests.
        try app.settle()
        if try app.exists(.button("Get started")) {
            try app.press(.button("Get started"))
        }
        return app
    }

    /// Settings → Accounts, from the mailbox, whichever Settings screen the app restores.
    private func openAccounts(_ app: App) throws {
        try ShowcaseApp.showMailbox(app)
        if !ShowcaseApp.pad {
            try app.press(.button(ShowcaseApp.folders))
        } else if try !app.appears(.button("Settings"), timeout: 3) {
            // In portrait the split view starts with its sidebar, and Settings in it, hidden.
            let toggles = try app.query(.id("ToggleSidebar")) + app.query(.button("Show Sidebar"))
            try app.activate(try XCTUnwrap(toggles.first, "no way to show the sidebar"))
        }
        // The Settings stack's back button is in the tree under the same name.
        let settings = try app.waitAll(.button("Settings"), "the folder list has no Settings")
            .first { $0.id != "BackButton" }
        try app.activate(try XCTUnwrap(settings, "the folder list has no Settings"))
        try app.wait(.button("Done"), "Settings did not open")

        // Settings opens on the screen it was last left on, which may be deeper than Accounts, and
        // the hub draws its rows lazily, so Accounts is there only once scrolled to.
        let deadline = Date().addingTimeInterval(ShowcaseApp.timeout)
        while try !app.appears(addAccount, timeout: 1), Date() < deadline {
            if let back = try app.query(.id("BackButton")).first, back.label != "Settings" {
                try app.tap(back)
            } else if let row = try accountsCategory(app) {
                try app.tap(row)
            } else {
                app.input.swipeUp()
            }
        }
        try app.wait(addAccount, timeout: 1, "Settings → Accounts did not open")
    }

    /// The Accounts category: the iPhone hub's row reads its summary after the name, the iPad
    /// sidebar's is the name alone.
    private func accountsCategory(_ app: App) throws -> Element? {
        try app.query(.textLike("Accounts")).first {
            $0.label == "Accounts" || ($0.role == "AXButton" && $0.label.hasPrefix("Accounts"))
        }
    }

    /// Alice's account of `kind` on the Accounts list: her two share an address.
    private func row(_ app: App, kind: String) throws -> Element? {
        try app.query(.id("settings-account-alice@test.local"))
            .first { $0.label.contains(", \(kind) ·") }
    }

    private func openPage(_ app: App, kind: String) throws {
        var found: Element?
        XCTAssertTrue(
            try wait(until: {
                found = try self.row(app, kind: kind)
                return found != nil
            }),
            "alice's \(kind) account is not listed"
        )
        try app.activate(try XCTUnwrap(found))
        // Until the push has finished, the page it came from is still drawn beside it.
        try app.settle()
    }

    private func back(_ app: App) throws {
        try ShowcaseApp.back(app)
        try app.wait(addAccount)
    }

    // MARK: Helpers

    /// Flips the switch off and waits for the question it asks first.
    ///
    /// Flipped again when nothing is asked: SwiftUI drops an alert asked for while the last one is
    /// still leaving, and nothing on screen says when that has finished. The switch stays on until
    /// the question is answered, so a second flip asks the same question rather than a different
    /// one.
    private func switchOff(_ toggle: Filter, asking question: Filter, in app: App) throws {
        for _ in 0..<3 {
            try app.toggle(try reveal(toggle, in: app), to: false)
            if try app.appears(question, timeout: 3) { return }
        }
        try app.wait(question, timeout: 1, "nothing was asked")
    }

    /// The element `filter` matches, scrolling the page until it is drawn.
    @discardableResult
    private func reveal(_ filter: Filter, in app: App) throws -> Element {
        for _ in 0..<8 {
            if let element = try app.query(filter).first { return element }
            app.input.swipeUp()
            try app.settle()
        }
        return try app.wait(filter, timeout: 1, "could not scroll to \(filter)")
    }

    private func wait(
        until condition: () throws -> Bool, for seconds: TimeInterval = ShowcaseApp.timeout
    ) throws -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if try condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }
        return try condition()
    }

    /// Whether something on this Mac accepts a connection on `port`: a simulator shares its
    /// loopback.
    private static func listening(on port: UInt16) -> Bool {
        let socket = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard socket >= 0 else { return false }
        defer { close(socket) }
        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = port.bigEndian
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        return withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(socket, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) == 0
            }
        }
    }
}
