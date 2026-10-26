// Adding a later account (`docs/settings.md`, row 9): Settings → Accounts carries the one add, and
// the mailbox carries none.
//
// The add closes Settings before the setup sheet opens, because a sheet presented over Settings is
// silently dropped. No package test sees that: it is two modals and their order, so it is driven
// here the way a person drives it.

import UIKit
import XCTest

@MainActor
final class SettingsAddAccountTests: XCTestCase {
    func testAccountsSettingsOpensAccountSetup() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "settings"])
        // The iPhone hub row reads its summary after the name; the iPad sidebar row is the name.
        let accounts = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Accounts'"))
            .firstMatch
        XCTAssertTrue(accounts.waitForExistence(timeout: ShowcaseApp.timeout))
        if !accounts.isHittable { app.swipeUp() }
        ShowcaseApp.tap(accounts)
        ShowcaseApp.tap(app.buttons["settings-add-account"])
        XCTAssertTrue(
            app.textFields["you@example.com"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Add account in Settings did not open account setup"
        )
    }

    func testTheMailboxHasNoAddOfItsOwn() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        // The folder list is lazy, so its last row exists only once it has been scrolled to: on
        // iPhone that means opening the drawer first.
        if UIDevice.current.userInterfaceIdiom != .pad {
            ShowcaseApp.tap(app.buttons["Folders"])
        }
        let folders = app.collectionViews.firstMatch
        XCTAssertTrue(folders.waitForExistence(timeout: ShowcaseApp.timeout))
        folders.swipeUp(velocity: .fast)
        folders.swipeUp(velocity: .fast)
        XCTAssertFalse(
            app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Add account'"))
                .firstMatch.exists,
            "a later account is added from Settings alone"
        )
    }
}
