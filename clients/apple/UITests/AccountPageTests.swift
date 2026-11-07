// An account's page in Settings → Accounts (docs/accounts.md), driven against the linked harness
// accounts rather than the showcase: alice's mailbox used for mail alone, and alice's and bob's
// calendar and contacts as accounts of their own. The showcase has no account its page could
// suggest linking, and a change on its page answers nothing a person would see.
//
// So this is the one class here that needs the local harness (`scripts/dev/harness.sh up`) and the
// certificate `test-ui.sh` hands the runner. Without either it skips and says why, and on a
// physical device it always skips: the harness is loopback-only.
//
// `stalwart-linked` connects its three canned accounts afresh on every launch, so the switch-off
// and the removal below are gone by the next run.

import Darwin
import UIKit
import XCTest

@MainActor
final class AccountPageTests: XCTestCase {
    func testUsesLinksAndRemovalOnAnAccountsPage() throws {
        let app = try launchOnTheLinkedHarness()
        openAccounts(app)

        // Alice's mailbox: her calendar account is suggested for both slots, and Link takes it.
        openPage(app, kind: "IMAP")
        let links = app.buttons.matching(NSPredicate(format: "label == 'Link'"))
        XCTAssertTrue(links.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout))
        XCTAssertEqual(links.count, 2, "a suggestion under each slot")
        XCTAssertTrue(
            app.staticTexts["Suggested: alice@test.local (CalDAV/CardDAV)"].exists,
            "the suggestion is not alice's calendar account"
        )
        reveal(links.firstMatch, in: app).tap()
        XCTAssertTrue(
            app.buttons["Calendar for invitations, alice@test.local (CalDAV/CardDAV)"]
                .waitForExistence(timeout: ShowcaseApp.timeout),
            "Link did not pick alice's calendar account"
        )
        XCTAssertTrue(
            wait(until: { links.count == 1 }), "the suggestion stayed after it was linked"
        )
        back(app)

        // Alice's calendar account: switching Contacts off asks first.
        openPage(app, kind: "CalDAV/CardDAV")
        let contacts = app.switches["Contacts"]
        XCTAssertTrue(contacts.waitForExistence(timeout: ShowcaseApp.timeout))
        XCTAssertEqual(contacts.value as? String, "1")
        let question = app.alerts["Switch off Contacts?"]
        flip(contacts, in: app)
        XCTAssertTrue(question.waitForExistence(timeout: ShowcaseApp.timeout), "nothing was asked")
        question.buttons["Cancel"].tap()
        XCTAssertTrue(question.waitForNonExistence(timeout: ShowcaseApp.timeout))
        XCTAssertEqual(contacts.value as? String, "1", "Cancel switched Contacts off")

        flip(contacts, in: app)
        XCTAssertTrue(question.waitForExistence(timeout: ShowcaseApp.timeout))
        question.buttons["Switch off"].tap()
        XCTAssertTrue(
            wait(until: { contacts.value as? String == "0" }), "Switch off left Contacts on"
        )
        // Calendar is now the account's last use, so it is held, and the page says why.
        let calendar = app.switches["Calendar"]
        XCTAssertEqual(calendar.value as? String, "1")
        XCTAssertTrue(wait(until: { !calendar.isEnabled }), "the last use can be switched off")
        XCTAssertTrue(
            app.staticTexts["An account is used for at least one of these. Remove it instead."]
                .exists,
            "the held switch does not say why"
        )

        // Removing it asks, names the mailbox that loses its link, and returns to the list.
        reveal(app.buttons["Remove account…"], in: app).tap()
        let removal = app.alerts["Remove account?"]
        XCTAssertTrue(removal.waitForExistence(timeout: ShowcaseApp.timeout), "nothing was asked")
        let unlinks = NSPredicate(
            format: "label CONTAINS %@", "It is also unlinked from alice@test.local (IMAP).")
        XCTAssertTrue(
            removal.staticTexts.matching(unlinks).firstMatch.exists,
            "the removal does not name the account that loses its link"
        )
        removal.buttons["Remove"].tap()
        XCTAssertTrue(
            app.buttons["settings-add-account"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the removal did not return to the list"
        )
        XCTAssertTrue(
            wait(until: { !self.row(app, kind: "CalDAV/CardDAV").exists }),
            "the removed account is still listed"
        )
        XCTAssertTrue(row(app, kind: "IMAP").exists, "the mailbox went with it")
    }

    // MARK: Getting there

    private func launchOnTheLinkedHarness() throws -> XCUIApplication {
        #if !targetEnvironment(simulator)
        throw XCTSkip("the harness is loopback-only, so a device cannot reach it")
        #else
        guard let ca = ProcessInfo.processInfo.environment["MAILCAL_EXTRA_CA"] else {
            throw XCTSkip(
                "no harness certificate: run this through clients/apple/Scripts/test-ui.sh")
        }
        for port: UInt16 in [12993, 28080] where !Self.listening(on: port) {
            throw XCTSkip("the harness is not up on port \(port): scripts/dev/harness.sh up")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(en)"]
        app.launchEnvironment = ["MAILCAL_DEV_ACCOUNT": "stalwart-linked", "MAILCAL_EXTRA_CA": ca]
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: ShowcaseApp.timeout))
        return app
        #endif
    }

    /// Settings → Accounts, from the mailbox, whichever Settings screen the app restores.
    private func openAccounts(_ app: XCUIApplication) {
        ShowcaseApp.showMailbox(app)
        // The Settings stack's back button is in the tree under the same name, over Folders.
        let settings = app.buttons
            .matching(NSPredicate(format: "label == 'Settings' AND identifier != 'BackButton'"))
            .firstMatch
        // Settings opens on the screen it was last left on, which may be deeper than Accounts.
        let add = app.buttons["settings-add-account"]
        let back = app.buttons["BackButton"]
        // The iPhone hub's row reads its summary after the name; the iPad sidebar's is a text.
        let categories = app.descendants(matching: .any).matching(NSPredicate(
            format: "(elementType == %d AND label BEGINSWITH 'Accounts') OR label == 'Accounts'",
            XCUIElement.ElementType.button.rawValue))
        if UIDevice.current.userInterfaceIdiom != .pad {
            ShowcaseApp.tap(app.buttons["Folders"])
            // ⚠️ The tab bar stays in the tree over the drawer's Settings row, so the row never
            // reads as hittable and is tapped where it is drawn, once the drawer is in.
            XCTAssertTrue(
                wait(until: { settings.frame.minX >= 0 }), "the folder drawer did not open"
            )
            sleep(1)
            settings.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        } else {
            // In portrait the split view starts with its sidebar, and Settings in it, hidden.
            if !settings.waitForExistence(timeout: 3) {
                ShowcaseApp.tap(app.buttons.matching(NSPredicate(
                    format: "identifier == 'ToggleSidebar' OR label == 'Show Sidebar'")).firstMatch)
            }
            ShowcaseApp.tap(settings)
        }
        XCTAssertTrue(
            app.buttons["Done"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Settings did not open"
        )
        // The hub draws its rows lazily, so Accounts is in the tree only once scrolled to.
        let deadline = Date().addingTimeInterval(ShowcaseApp.timeout)
        while !add.waitForExistence(timeout: 1), Date() < deadline {
            if back.exists, back.label != "Settings" {
                back.tap()
            } else if let row = categories.allElementsBoundByIndex.first(where: \.isHittable) {
                row.tap()
            } else {
                app.swipeUp()
            }
        }
        XCTAssertTrue(add.exists, "Settings → Accounts did not open")
    }

    /// Alice's account of `kind` on the Accounts list: her two share an address.
    private func row(_ app: XCUIApplication, kind: String) -> XCUIElement {
        app.buttons.matching(identifier: "settings-account-alice@test.local")
            .matching(NSPredicate(format: "label CONTAINS %@", ", \(kind) ·")).firstMatch
    }

    private func openPage(_ app: XCUIApplication, kind: String) {
        ShowcaseApp.tap(row(app, kind: kind))
    }

    private func back(_ app: XCUIApplication) {
        ShowcaseApp.tap(app.buttons["BackButton"])
        XCTAssertTrue(
            app.buttons["settings-add-account"].waitForExistence(timeout: ShowcaseApp.timeout)
        )
    }

    // MARK: Helpers

    /// Taps the switch itself: the row around it carries the label, and a tap on its middle lands
    /// on the text.
    private func flip(_ toggle: XCUIElement, in app: XCUIApplication) {
        reveal(toggle.switches.firstMatch, in: app).tap()
    }

    /// Scrolls the page until `element` can be tapped.
    private func reveal(_ element: XCUIElement, in app: XCUIApplication) -> XCUIElement {
        XCTAssertTrue(
            element.waitForExistence(timeout: ShowcaseApp.timeout), "never appeared: \(element)"
        )
        for _ in 0..<8 where !element.isHittable {
            app.swipeUp()
        }
        XCTAssertTrue(element.isHittable, "could not scroll to: \(element)")
        return element
    }

    private func wait(
        until condition: () -> Bool, for seconds: TimeInterval = ShowcaseApp.timeout
    ) -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.25))
        }
        return condition()
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
