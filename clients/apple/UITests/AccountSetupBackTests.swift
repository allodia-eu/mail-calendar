// The way back from the second step of account setup (`docs/account-autodetect.md`, rule 12).
//
// The found card and the manual form each carry Back, and Back returns to the address with what
// was typed there. No package test reaches either step: which one appears is decided by the core's
// detection, which the showcase build answers from a script keyed on the domain, so the steps are
// reached here the way a person reaches them.
//
// Back is found by its identifier rather than its label: behind a later add's sheet the calendar's
// previous-week chevron is also a button labelled "Back".

import XCTest

@MainActor
final class AccountSetupBackTests: XCTestCase {
    /// A later add, landing on the found card for the showcase's trusted domain.
    func testFoundCardGoesBackToTheAddress() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-detected"])
        XCTAssertTrue(
            app.staticTexts["We found your settings"].waitForExistence(timeout: ShowcaseApp.timeout),
            "detection did not land on the found card"
        )
        XCTAssertTrue(
            app.buttons["Cancel"].exists,
            "a later add's found card needs the Cancel the other two steps carry"
        )
        ShowcaseApp.tap(app.buttons["setup-back"])
        assertOnAddressStep(app, holding: "eva@northwind.example")
    }

    /// A later add, landing on the manual form because the domain published nothing.
    func testManualFormGoesBackToTheAddress() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-manual"])
        ShowcaseApp.tap(app.buttons["setup-back"])
        assertOnAddressStep(app, holding: "eva.jansen@example.com")
    }

    /// The first run, which has no Cancel and so, before Back, no way off the manual form at all.
    func testFirstRunManualFormGoesBackToTheAddress() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        let address = "someone@example.com"
        let field = app.textFields["you@example.com"]
        ShowcaseApp.tap(field)
        field.typeText(address)
        ShowcaseApp.tap(app.buttons["Set up manually"])
        ShowcaseApp.tap(app.buttons["setup-back"])
        assertOnAddressStep(app, holding: address)
    }

    private func assertOnAddressStep(
        _ app: XCUIApplication,
        holding address: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let field = app.textFields["you@example.com"]
        XCTAssertTrue(
            field.waitForExistence(timeout: ShowcaseApp.timeout),
            "Back did not return to the address step", file: file, line: line
        )
        XCTAssertEqual(
            field.value as? String, address,
            "going back must keep the address", file: file, line: line
        )
        XCTAssertFalse(
            app.buttons["setup-back"].exists,
            "the address step is the first step and has nothing to go back to", file: file, line: line
        )
    }
}
