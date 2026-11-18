// The composer's Importance menu (`RichComposerView.Importance.swift`, docs/importance.md): it offers
// High, Normal and Low, marks the choice in force, and the button says which one that is.
//
// The showcase seeds no message sent with high or low importance, so a row's mark and the reading
// banner are not reached from here.

import XCTest

@MainActor
final class ComposerImportanceTests: XCTestCase {
    private let choices = ["High", "Normal", "Low"]

    func testTheMenuOffersThreeChoicesAndMarksThePick() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        try app.wait(.button("Send"), "Compose did not open the composer")

        let importance = try app.wait(.button("Importance"), "no Importance menu")
        XCTAssertEqual(importance.value, "Normal", "a new message opens away from Normal")

        try open(importance, in: app)
        for choice in choices {
            XCTAssertTrue(try app.exists(.button(choice)), "the menu offers no \(choice)")
        }
        XCTAssertEqual(try marked(in: app), ["Normal"], "the menu does not mark Normal")
        try app.press(.button("High"))
        XCTAssertTrue(try says("High", in: app), "the button does not say High")

        try open(importance, in: app)
        XCTAssertEqual(try marked(in: app), ["High"], "the reopened menu does not mark the pick")
        try app.press(.button("Low"))
        XCTAssertTrue(try says("Low", in: app), "the button does not say Low")

        try app.press(.button("More actions"))
        try app.wait(.button("Save as draft"), "More holds no Save as draft")
        try app.press(.button("Discard"))
        // An untouched message closes at once; one with something to lose asks first.
        if try !app.goes(.button("Send"), timeout: 2) {
            try app.press(.button("Discard"))
        }
        try app.wait(.button("New Mail"), "Discard did not close the composer")
    }

    /// Opens the menu and waits for its entries, opening it again if SwiftUI closed it at once.
    private func open(_ menu: Element, in app: App) throws {
        for _ in 1...3 {
            try app.activate(menu)
            if try app.appears(.button("Low"), timeout: 5) { return }
        }
        XCTFail("the Importance menu did not open")
    }

    /// The menu entries drawn with a checkmark.
    private func marked(in app: App) throws -> [String] {
        try choices.filter { choice in
            try app.query(.button(choice)).contains { $0.selected }
        }
    }

    /// Whether the button comes to say `choice`: SwiftUI redraws it once the core has answered.
    private func says(_ choice: String, in app: App) throws -> Bool {
        let deadline = Date().addingTimeInterval(ShowcaseApp.timeout)
        repeat {
            if try app.wait(.button("Importance")).value == choice { return true }
            try app.settle()
        } while Date() < deadline
        return false
    }
}
