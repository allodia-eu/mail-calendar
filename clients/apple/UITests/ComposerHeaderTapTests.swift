// A phone composer's header takes its own taps, and the message below it takes the rest
// (`TouchComposerChrome`).
//
// The header is hosted inside the web view's scroll view, over the page, and lets every touch
// outside itself through to the message. SwiftUI draws a plain button inside its hosting view
// rather than as a view of its own, so a pass-through that asked UIKit what was under the finger
// saw the paperclip as empty space and focused the message instead. Nothing below the UI can see
// where a touch lands, so this is the only gate on it.

import XCTest

@MainActor
final class ComposerHeaderTapTests: XCTestCase {
    func testTheHeadersButtonsTakeTheirTaps() {
        let app = openComposer()

        ShowcaseApp.tap(app.buttons["Show Cc and Bcc"])
        XCTAssertTrue(
            app.textFields["Cc"].waitForExistence(timeout: 5), "the chevron did not open Cc and Bcc"
        )

        ShowcaseApp.tap(app.buttons["More actions"])
        XCTAssertTrue(
            app.buttons["Save as draft"].waitForExistence(timeout: 5), "More did not open its menu"
        )
        dismissMenu(app)

        // The file picker is drawn by another process, so nothing of it is in this app's tree: what
        // shows it opened is that it covers the composer. A tap the message took instead leaves the
        // paperclip where it was, with the keyboard up under it.
        let attach = app.buttons["Attach files"]
        ShowcaseApp.tap(attach)
        let covered = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isHittable == false"), object: attach
        )
        XCTAssertEqual(
            XCTWaiter().wait(for: [covered], timeout: ShowcaseApp.timeout), .completed,
            "the paperclip did not open the file picker"
        )
    }

    func testATapBelowTheHeaderWritesInTheMessage() {
        let app = openComposer()
        let subject = app.textFields["Subject"]
        XCTAssertTrue(subject.waitForExistence(timeout: 5), "the composer has no Subject")

        // Below the action bar, in the part of the page the header leaves to the message.
        let bar = app.buttons["Attach files"].frame
        app.windows.firstMatch
            .coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: bar.minX + 20, dy: bar.maxY + 80))
            .tap()
        app.typeText("below")

        let web = app.webViews.firstMatch
        XCTAssertTrue(
            web.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "below")).firstMatch
                .waitForExistence(timeout: 5),
            "a tap below the header did not put the caret in the message"
        )
        XCTAssertEqual(subject.value as? String, "Subject", "the tap reached the header instead")
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

    /// Closes an open menu by tapping outside it, on the navigation bar's title, which does
    /// nothing else.
    private func dismissMenu(_ app: XCUIApplication) {
        app.navigationBars["New Message"].staticTexts["New Message"].tap()
        XCTAssertTrue(
            app.buttons["Save as draft"].waitForNonExistence(timeout: 5), "the menu did not close"
        )
    }
}
