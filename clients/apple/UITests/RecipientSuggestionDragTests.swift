// Scrolling a recipient field's suggestion list never picks an address (docs/contacts.md §4).
//
// A row that is a SwiftUI `Button` fires when the finger lifts anywhere inside it, and a slow drag
// through the list is not always taken by the scroll view first, so scrolling picked the address
// the finger came to rest on. Nothing below the UI can see a gesture, so the drag and the tap here
// are real ones. In landscape, because that is where the composer's list is too long for the room
// above the keyboard and scrolls: upright, the showcase's suggestions all fit.

import XCTest

@MainActor
final class RecipientSuggestionDragTests: XCTestCase {
    override func tearDown() async throws {
        XCUIDevice.shared.orientation = .portrait
    }

    func testScrollingTheSuggestionsPicksNothingAndATapPicksOne() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        XCUIDevice.shared.orientation = .landscapeLeft
        try app.press(.button("New Mail"))
        try app.wait(.text("New Message").role("AXHeading"), "Compose did not open the composer")

        // The composer opens with the caret in To.
        app.input.typeText("e")
        guard let first = onScreenSuggestion(app) else { return XCTFail("To offered no suggestions") }

        // A slow drag up through the list, as a finger reading down it would make.
        let start = first.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        start.press(
            forDuration: 0.1,
            thenDragTo: start.withOffset(CGVector(dx: 0, dy: -80)),
            withVelocity: .slow,
            thenHoldForDuration: 0.3
        )
        XCTAssertFalse(try app.appears(anyPill, timeout: 2), "scrolling the suggestions picked one")

        guard let suggestion = onScreenSuggestion(app) else { return XCTFail("the list closed") }
        suggestion.tap()
        try app.wait(anyPill, "a tap on a suggestion did not pick it")
    }

    /// A suggestion on screen, found through XCUITest. The list is drawn two ways (rows, and the
    /// same rows in a scroll view) and both are on screen to the agent; only a hit test tells which
    /// one a finger reaches.
    private func onScreenSuggestion(_ app: App) -> XCUIElement? {
        let suggestions = app.input.buttons.matching(NSPredicate(format: "label CONTAINS %@", "@"))
        guard suggestions.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout) else { return nil }
        return suggestions.allElementsBoundByIndex.first(where: \.isHittable)
    }

    /// The remove control of a picked address's pill, which To holds once anything is picked.
    private let anyPill = Filter.textLike(", Remove recipient").role("AXButton")
}
