// The composer's Importance menu (`RichComposerView.Importance.swift`, docs/importance.md): it offers
// High, Normal and Low, marks the choice in force, and the button says which one that is.
//
// The showcase seeds no message sent with high or low importance, so a row's mark and the reading
// banner are not reached from here.

import XCTest

@MainActor
final class ComposerImportanceTests: XCTestCase {
    func testTheMenuOffersThreeChoicesAndMarksThePick() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        XCTAssertTrue(app.buttons["Send"].waitForExistence(timeout: ShowcaseApp.timeout))

        let importance = app.buttons["Importance"].firstMatch
        XCTAssertTrue(importance.waitForExistence(timeout: ShowcaseApp.timeout), "no Importance menu")
        XCTAssertEqual(importance.value as? String, "Normal", "a new message opens away from Normal")

        open(importance, in: app)
        for choice in ["High", "Normal", "Low"] {
            XCTAssertTrue(app.buttons[choice].exists, "the menu offers no \(choice)")
        }
        XCTAssertEqual(marked(in: app), ["Normal"], "the menu does not mark Normal")
        ShowcaseApp.tap(app.buttons["High"])
        XCTAssertTrue(
            wait(until: { importance.value as? String == "High" }), "the button does not say High"
        )

        open(importance, in: app)
        XCTAssertEqual(marked(in: app), ["High"], "the reopened menu does not mark the pick")
        ShowcaseApp.tap(app.buttons["Low"])
        XCTAssertTrue(
            wait(until: { importance.value as? String == "Low" }), "the button does not say Low"
        )

        ShowcaseApp.tap(app.buttons["More actions"])
        ShowcaseApp.tap(app.buttons["Discard"])
        let confirm = app.alerts.buttons["Discard"]
        if confirm.waitForExistence(timeout: 2) { confirm.tap() }
        XCTAssertTrue(
            app.buttons["New Mail"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Discard did not close the composer"
        )
    }

    /// Opens the menu and waits for its entries.
    private func open(_ menu: XCUIElement, in app: XCUIApplication) {
        ShowcaseApp.tap(menu)
        XCTAssertTrue(app.buttons["Low"].waitForExistence(timeout: ShowcaseApp.timeout), "the menu did not open")
    }

    /// The menu entries drawn with a checkmark.
    private func marked(in app: XCUIApplication) -> [String] {
        app.buttons
            .matching(NSPredicate(format: "label IN %@ AND isSelected == true", ["High", "Normal", "Low"]))
            .allElementsBoundByIndex
            .map(\.label)
    }

    private func wait(until condition: @escaping () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(ShowcaseApp.timeout)
        while Date() < deadline {
            if condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        }
        return false
    }
}
