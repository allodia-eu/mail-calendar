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
    private let address = Filter.text("you@example.com").role("AXTextField")
    private let back = Filter.id("setup-back")

    /// A later add, landing on the found card for the showcase's trusted domain.
    func testFoundCardGoesBackToTheAddress() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-detected"])
        try app.wait(.text("We found your settings"), "detection did not land on the found card")
        XCTAssertTrue(
            try app.exists(.button("Cancel")),
            "a later add's found card needs the Cancel the other two steps carry"
        )
        try app.press(back)
        try assertOnAddressStep(app, holding: "eva@northwind.example")
    }

    /// A later add, landing on the manual form because the domain published nothing.
    func testManualFormGoesBackToTheAddress() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-manual"])
        try app.press(back)
        try assertOnAddressStep(app, holding: "eva.jansen@example.com")
    }

    /// The first run, which has no Cancel and so, before Back, no way off the manual form at all.
    func testFirstRunManualFormGoesBackToTheAddress() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        let typed = "someone@example.com"
        try app.fill(address, typed)
        try app.press(.button("Set up manually"))
        try app.press(back)
        try assertOnAddressStep(app, holding: typed)
    }

    private func assertOnAddressStep(
        _ app: App,
        holding typed: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        let field = try app.wait(address, "Back did not return to the address step")
        XCTAssertEqual(field.value, typed, "going back must keep the address", file: file, line: line)
        XCTAssertFalse(
            try app.exists(back),
            "the address step is the first step and has nothing to go back to", file: file, line: line
        )
    }
}
