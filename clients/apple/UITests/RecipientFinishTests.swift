// Return and leaving the field finish an address as a comma does (docs/contacts.md): a phone's email
// keyboard has no comma on its first page.

import XCTest

@MainActor
final class RecipientFinishTests: XCTestCase {
    func testReturnFinishesTheAddressAndKeepsTheField() {
        let app = openComposer()
        // The composer opens with the caret in To.
        type("bob@example.test\n", in: app)
        XCTAssertTrue(pill("bob@example.test", in: app).waitForExistence(timeout: 5), "no pill")
        XCTAssertEqual(app.textFields["To"].value as? String, "To", "the address stayed text")

        type("carol@example.test\n", in: app)
        XCTAssertTrue(
            pill("carol@example.test", in: app).waitForExistence(timeout: 5),
            "Return took the keyboard out of the field"
        )
    }

    func testLeavingTheFieldFinishesTheAddress() {
        let app = openComposer()
        type("bob@example.test", in: app)
        ShowcaseApp.tap(app.textFields["Subject"])
        XCTAssertTrue(pill("bob@example.test", in: app).waitForExistence(timeout: 5), "no pill")
    }

    private func openComposer() -> XCUIApplication {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        XCTAssertTrue(
            app.webViews.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout),
            "the composer drew no editor"
        )
        return app
    }

    /// One key at a time, as `ComposerUndoTests` types into To, and for its reason.
    private func type(_ text: String, in app: XCUIApplication) {
        for letter in text {
            app.typeText(String(letter))
        }
    }

    /// A finished address's remove control, which names it.
    private func pill(_ address: String, in app: XCUIApplication) -> XCUIElement {
        app.buttons["\(address), Remove recipient"]
    }
}
