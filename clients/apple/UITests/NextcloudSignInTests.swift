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
    func testTheFoundCardLeadsWithNextcloudAndPutsThePasswordBehindIt() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        let field = app.textFields["you@example.com"]
        XCTAssertTrue(field.waitForExistence(timeout: ShowcaseApp.timeout))
        ShowcaseApp.tap(field)
        field.typeText("eva@nextcloud.example")
        ShowcaseApp.tap(app.buttons["Continue"])

        let signIn = app.buttons["Sign in with Nextcloud"]
        XCTAssertTrue(
            signIn.waitForExistence(timeout: ShowcaseApp.timeout),
            "a Nextcloud server's card did not offer its sign-in"
        )
        XCTAssertTrue(
            app.staticTexts.matching(
                NSPredicate(format: "label BEGINSWITH 'This server runs Nextcloud.'")
            ).firstMatch.exists,
            "the line explaining the sign-in is missing"
        )
        let password = app.secureTextFields["Password"]
        XCTAssertFalse(password.exists, "the password was drawn beside a sign-in that leads")
        XCTAssertFalse(app.buttons["Connect"].exists, "Connect was offered with no password field")

        ShowcaseApp.tap(app.buttons["Use a password instead"])
        XCTAssertTrue(
            password.waitForExistence(timeout: ShowcaseApp.timeout),
            "Use a password instead drew no password field"
        )
        XCTAssertTrue(signIn.exists, "the sign-in went away once a password was chosen")
        XCTAssertFalse(app.buttons["Use a password instead"].exists)
    }

    /// The same detection without Nextcloud stays the password card.
    func testAnotherCalendarServersCardOffersNoNextcloudSignIn() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        let field = app.textFields["you@example.com"]
        XCTAssertTrue(field.waitForExistence(timeout: ShowcaseApp.timeout))
        ShowcaseApp.tap(field)
        field.typeText("eva@cloud.example")
        ShowcaseApp.tap(app.buttons["Continue"])
        XCTAssertTrue(
            app.secureTextFields["Password"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the calendar-and-contacts card drew no password field"
        )
        XCTAssertFalse(app.buttons["Sign in with Nextcloud"].exists)
    }
}
