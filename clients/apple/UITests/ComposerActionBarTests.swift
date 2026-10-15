// The touch composer's header fits a phone: the From label keeps its width, and the action bar is
// symbols in one row with the draft's actions behind More (`RichComposerView.ActionBar.swift`).
//
// Run on a phone with two accounts and a signature, which is what the showcase is: the sender
// picker and the signature menu both appear only then, and with them a header too wide for the
// screen squeezed "From" to a letter a line and wrapped every button's title.

import XCTest

@MainActor
final class ComposerActionBarTests: XCTestCase {
    func testTheHeaderFitsAPhoneAndKeepsEveryAction() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        XCTAssertTrue(app.buttons["Send"].waitForExistence(timeout: ShowcaseApp.timeout))

        let from = app.staticTexts["From"]
        XCTAssertGreaterThan(from.frame.width, from.frame.height, "From is squeezed into a column")

        let screen = app.windows.firstMatch.frame
        for name in ["Attach files", "Signature", "More actions"] {
            let button = app.buttons[name]
            XCTAssertTrue(button.exists, "the action bar has no \(name)")
            XCTAssertTrue(screen.contains(button.frame), "\(name) runs off the screen")
            XCTAssertLessThanOrEqual(button.frame.height, 50, "\(name) wraps onto more lines")
        }

        ShowcaseApp.tap(app.buttons["More actions"])
        XCTAssertTrue(
            app.buttons["Save as draft"].waitForExistence(timeout: ShowcaseApp.timeout),
            "More holds no Save as draft"
        )
        ShowcaseApp.tap(app.buttons["Discard"])
        // An untouched message closes at once; one with something to lose asks first.
        let confirm = app.alerts.buttons["Discard"]
        if confirm.waitForExistence(timeout: 2) { confirm.tap() }
        XCTAssertTrue(
            app.buttons["New Mail"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Discard did not close the composer"
        )
    }
}
