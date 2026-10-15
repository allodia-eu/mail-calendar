// Scrolling a recipient field's suggestion list never picks an address (docs/contacts.md §4).
//
// A row that is a SwiftUI `Button` fires when the finger lifts anywhere inside it, and a slow drag
// through the list is not always taken by the scroll view first, so scrolling picked the address
// the finger came to rest on. Nothing below the UI can see a gesture, so this is the only gate on
// it. In landscape, because that is where the composer's list is too long for the room above the
// keyboard and scrolls: upright, the showcase's suggestions all fit.

import XCTest

@MainActor
final class RecipientSuggestionDragTests: XCTestCase {
    override func tearDown() async throws {
        XCUIDevice.shared.orientation = .portrait
    }

    func testScrollingTheSuggestionsPicksNothingAndATapPicksOne() {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        XCUIDevice.shared.orientation = .landscapeLeft
        ShowcaseApp.tap(app.buttons["New Mail"])
        XCTAssertTrue(
            app.navigationBars["New Message"].waitForExistence(timeout: ShowcaseApp.timeout),
            "Compose did not open the composer"
        )

        // The composer opens with the caret in To.
        app.typeText("e")
        guard let first = onScreenSuggestion(app) else { return XCTFail("To offered no suggestions") }

        // A slow drag up through the list, as a finger reading down it would make.
        let start = first.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        start.press(
            forDuration: 0.1,
            thenDragTo: start.withOffset(CGVector(dx: 0, dy: -80)),
            withVelocity: .slow,
            thenHoldForDuration: 0.3
        )
        XCTAssertFalse(
            anyPill(app).waitForExistence(timeout: 2), "scrolling the suggestions picked one"
        )

        guard let suggestion = onScreenSuggestion(app) else { return XCTFail("the list closed") }
        ShowcaseApp.tap(suggestion)
        XCTAssertTrue(
            anyPill(app).waitForExistence(timeout: ShowcaseApp.timeout),
            "a tap on a suggestion did not pick it"
        )
    }

    /// A suggestion on screen. The list is drawn two ways (rows, and the same rows in a scroll
    /// view) and the accessibility tree holds both, the unused one off screen.
    private func onScreenSuggestion(_ app: XCUIApplication) -> XCUIElement? {
        let suggestions = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "@"))
        guard suggestions.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout) else { return nil }
        return suggestions.allElementsBoundByIndex.first(where: \.isHittable)
    }

    /// The remove control of a picked address's pill, which To holds once anything is picked.
    private func anyPill(_ app: XCUIApplication) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label ENDSWITH %@", ", Remove recipient")).firstMatch
    }
}
