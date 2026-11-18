// A phone composer's header takes its own taps, and the message below it takes the rest
// (`TouchComposerChrome`).
//
// The header is hosted inside the web view's scroll view, over the page, and lets every touch
// outside itself through to the message. SwiftUI draws a plain button inside its hosting view
// rather than as a view of its own, so a pass-through that asked UIKit what was under the finger
// saw the paperclip as empty space and focused the message instead. Nothing below the UI can see
// where a touch lands, so every press here is a finger, aimed where the agent says the control is.

import XCTest

@MainActor
final class ComposerHeaderTapTests: XCTestCase {
    func testTheHeadersButtonsTakeTheirTaps() throws {
        let app = try openComposer()

        try app.tap(app.wait(.button("Show Cc and Bcc")))
        try app.wait(
            .text("Cc").role("AXTextField"), timeout: 5, "the chevron did not open Cc and Bcc"
        )

        try app.tap(app.wait(.button("More actions")))
        try app.wait(.button("Save as draft"), timeout: 5, "More did not open its menu")
        try dismissMenu(app)

        // The paperclip opens its menu of sources. The file picker behind Files is drawn by another
        // process, so nothing of it is in the app's own views and the agent sees the composer
        // unchanged under it: what shows it opened is that XCUITest, which reads the picker too,
        // finds the paperclip covered. A tap the message took instead leaves the paperclip where
        // it was, with the keyboard up under it.
        try app.tap(app.wait(.button("Attach files")))
        try app.tap(app.wait(.button("Files"), timeout: 5, "the paperclip did not open its menu"))
        let covered = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isHittable == false"),
            object: app.input.buttons["Attach files"]
        )
        XCTAssertEqual(
            XCTWaiter().wait(for: [covered], timeout: ShowcaseApp.timeout), .completed,
            "the paperclip did not open the file picker"
        )
    }

    /// Tapped once the message's editor is there: the web view exists before its page has loaded,
    /// and a tap on a page with no editor in it focuses nothing, which leaves the caret in To.
    func testATapBelowTheHeaderWritesInTheMessage() throws {
        let app = try openComposer()
        let subject = Filter.text("Subject").role("AXTextField")
        try app.wait(subject, timeout: 5, "the composer has no Subject")
        let web = app.input.webViews.firstMatch
        XCTAssertTrue(
            web.textViews.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout),
            "the message has no editor"
        )

        // Below the action bar, in the part of the page the header leaves to the message.
        try app.point(app.wait(.button("Attach files")), at: CGVector(dx: 0, dy: 1))
            .withOffset(CGVector(dx: 20, dy: 80))
            .tap()
        app.input.typeText("below")

        XCTAssertTrue(
            web.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "below")).firstMatch
                .waitForExistence(timeout: 5),
            "a tap below the header did not put the caret in the message"
        )
        XCTAssertEqual(try app.wait(subject).value, "", "the tap reached the header instead")
        XCTAssertFalse(
            (try app.wait(.text("To").role("AXTextField")).value ?? "").contains("below"),
            "the tap left the caret in To"
        )
    }

    private func openComposer() throws -> App {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        try app.wait(Filter().type("WKWebView"), "the composer drew no editor")
        return app
    }

    /// Closes an open menu by tapping outside it, on the navigation bar's title, which does
    /// nothing else.
    private func dismissMenu(_ app: App) throws {
        try app.tap(app.wait(.text("New Message").role("AXHeading")))
        try app.waitGone(.button("Save as draft"), "the menu did not close")
    }
}
