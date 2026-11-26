// The manual form's Nextcloud type (docs/nextcloud-sign-in.md rule 2): a pane that asks for the
// server's address alone, and what it says when the core could not place that address.
//
// The core asks the address typed for real. Nothing listens on port 9 of the loopback address, so
// the check finds nothing either way there and answers unsure; trying anyway is then refused by
// the same silence. A Nextcloud server's answer opens a browser, which is not driven here: the
// wait and its Cancel are the found card's, pinned by the package suite's state tests.

import XCTest

@MainActor
final class AccountSetupNextcloudTests: XCTestCase {
    private let server = Filter.text("Server address, such as cloud.example.com").role("AXTextField")
    private let signIn = Filter.button("Sign in with Nextcloud")
    private let tryAnyway = Filter.button("Try anyway")
    private let silent = "127.0.0.1:9"

    func testThePaneAsksForTheServersAddressAlone() throws {
        let app = try openNextcloudPane()
        try app.wait(server, "the Nextcloud pane drew no address field")
        try app.wait(signIn.enabled(false), "the sign-in could be pressed with no address")
        XCTAssertFalse(try app.exists(.text("Password").role("AXTextField")), "a password was asked for")
        XCTAssertFalse(try app.exists(.button("Connect")), "Connect was offered beside the sign-in")
        try app.fill(server, "cloud.test.local")
        try app.wait(signIn.enabled(true), "an address typed did not enable the sign-in")
    }

    func testAnAddressThatCannotBeAServersSaysSo() throws {
        let app = try openNextcloudPane()
        try app.fill(server, "eva:secret@cloud.test.local")
        try app.press(signIn)
        try app.wait(
            .textLike("Enter the address of your Nextcloud server, such as"),
            "an address carrying a password was not refused"
        )
        XCTAssertEqual(
            try app.wait(server, "the address field went away").value,
            "eva:secret@cloud.test.local", "the address typed was not kept"
        )
    }

    func testAnUnsureAddressOffersTryAnywayWhileTheFieldHoldsIt() throws {
        let app = try openNextcloudPane()
        try app.fill(server, silent)
        try app.press(signIn)
        try app.wait(.textLike("Could not confirm that"), "the check did not say it was unsure")
        try app.wait(tryAnyway, "an unsure check offered no Try anyway")

        try app.fill(server, "cloud.test.local")
        XCTAssertTrue(try app.goes(tryAnyway), "Try anyway stayed for an address nobody asked about")
        try app.fill(server, silent)
        try app.wait(tryAnyway, "Try anyway did not come back with the address asked")

        try app.press(tryAnyway)
        try app.wait(
            .textLike("No Nextcloud server answered at"), "trying anyway did not say it failed"
        )
        XCTAssertFalse(try app.exists(.textLike("Could not confirm that")), "the unsure note stayed")
        XCTAssertEqual(
            try app.wait(server, "the address field went away").value, silent,
            "the address typed was not kept"
        )
    }

    private func openNextcloudPane() throws -> App {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-manual"])
        let menus = try app.waitAll(.textLike("Account type").role("AXButton"), "no account type menu")
        let menu = try XCTUnwrap(
            menus.first { $0.label.hasPrefix("Account type") }, "no account type menu"
        )
        try app.choose(.button("Nextcloud"), from: menu, "the menu offered no Nextcloud")
        return app
    }
}
