// Signing in with Nextcloud on the found card (docs/nextcloud-sign-in.md rule 2): at a server
// detection recognised as Nextcloud the sign-in leads under the line that explains it, and the
// password waits behind "Use a password instead".
//
// The showcase answers detection for `nextcloud.example` with such a server. The browser half
// is not driven here: the showcase has no server to start a sign-in at, so the wait and its
// Cancel are pinned by the package suite's state tests and verified by hand against a server.

import XCTest

@MainActor
final class NextcloudSignInTests: XCTestCase {
    private let signIn = Filter.button("Sign in with Nextcloud")
    private let password = Filter.text("Password").role("AXTextField")
    private let usePassword = Filter.button("Use a password instead")

    func testTheFoundCardLeadsWithNextcloudAndPutsThePasswordBehindIt() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        try detect("eva@nextcloud.example", in: app)

        try app.wait(signIn, "a Nextcloud server's card did not offer its sign-in")
        let explained = "This server runs Nextcloud."
        XCTAssertTrue(
            try app.query(.textLike(explained)).contains { $0.label.hasPrefix(explained) },
            "the line explaining the sign-in is missing"
        )
        XCTAssertFalse(
            try app.exists(password), "the password was drawn beside a sign-in that leads"
        )
        XCTAssertFalse(
            try app.exists(.button("Connect")), "Connect was offered with no password field"
        )

        try app.press(usePassword)
        try app.wait(password, "Use a password instead drew no password field")
        XCTAssertTrue(try app.exists(signIn), "the sign-in went away once a password was chosen")
        XCTAssertFalse(try app.exists(usePassword))
    }

    /// The same detection without Nextcloud stays the password card.
    func testAnotherCalendarServersCardOffersNoNextcloudSignIn() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        try detect("eva@cloud.example", in: app)
        try app.wait(password, "the calendar-and-contacts card drew no password field")
        XCTAssertFalse(try app.exists(signIn))
    }

    private func detect(_ address: String, in app: App) throws {
        try app.fill(.text("you@example.com").role("AXTextField"), address)
        try app.press(.button("Continue"))
    }
}
