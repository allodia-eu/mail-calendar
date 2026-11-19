// The own AI endpoint's form under Settings → Advanced (`docs/ai.md` rules 4 to 7), in the running
// app. A debug build offers the writing assistant to everyone, so the form is there without an
// Allodia account. What a save refuses is asked of the core as the person types and Save is held
// meanwhile; what a save came to is said once.
//
// The showcase core keeps no credential, so a key is refused there as a keystore would refuse it;
// a key that is kept is the core's test (`app_ai_tests.rs`). Nothing is dialled: saving stores the
// fields, and only the first request would reach the server, which these tests never make.

import UIKit
import XCTest

@MainActor
final class AiEndpointSettingsTests: XCTestCase {
    private let address = Filter.id("aiEndpoint.address")
    private let model = Filter.id("aiEndpoint.model")
    private let key = Filter.id("aiEndpoint.key")
    private let save = Filter.id("aiEndpoint.save").role("AXButton")
    private let remove = Filter.id("aiEndpoint.remove").role("AXButton")
    private let euNative = "In the EU, run by a European company, or on my own computer"

    func testTheFormIsOfferedAndSaveIsHeldWhileAGapIsNamed() throws {
        let app = try openAdvanced()
        try app.wait(save.enabled(false), "Save was not held with every field empty")
        XCTAssertFalse(
            try app.exists(.text("Say where the server runs.")),
            "a form nobody has started on names its gaps")

        try app.fill(address, "http://localhost:11434/v1")
        try app.fill(model, "mistral")
        try app.wait(.text("Say where the server runs."), "the missing declaration was not said")
        try app.wait(save.enabled(false), "Save was not held without a declaration")

        try app.press(.button(euNative), "the declaration has no EU-native answer")
        try app.wait(save.enabled(true), "Save stayed held with nothing missing")

        try app.fill(address, "http://ai.test.local/v1")
        try app.wait(
            .text("Use an https address, unless the server runs on this computer."),
            "an address elsewhere without HTTPS was not refused beside its field")
        try app.wait(save.enabled(false), "Save was not held for an address needing HTTPS")
    }

    func testASaveRunsToItsOutcomeAndSaysItOnce() throws {
        let app = try openAdvanced()
        try app.fill(address, "http://localhost:11434/v1")
        try app.fill(model, "mistral")
        try app.press(.button(euNative))

        // The showcase keeps no key, so the keystore refuses one: the form says so and keeps
        // the key as typed.
        try app.fill(key, "sk-test")
        try app.press(save.enabled(true), "Save could not be pressed")
        try app.wait(.text("The key could not be stored."), "a refused key was not said")
        XCTAssertFalse(
            (try app.wait(key).value ?? "").isEmpty, "the refused key was taken out of its field")

        // The next change takes the line away, and a save without a key is kept. The refusal
        // before it is not said again.
        try app.fill(key, "")
        try app.press(save.enabled(true))
        try app.wait(remove.enabled(true), "a kept endpoint offered no Remove")
        try app.settle()
        XCTAssertFalse(
            try app.exists(.text("The key could not be stored.")), "an outcome was said twice")
        XCTAssertTrue(
            try app.exists(.text("Leave empty if the server needs none.")),
            "the form said a key is kept when none is")

        try app.press(remove.enabled(true))
        try app.wait(remove.enabled(false), "Remove stayed offered after the removal")
        XCTAssertEqual(try app.wait(address).value ?? "", "", "the removed address is still drawn")
    }

    /// Settings → Advanced, scrolled to the form.
    private func openAdvanced() throws -> App {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "settings"])
        try app.wait(.text("Settings").role("AXHeading"), "Settings did not open")
        var row = try advancedRow(app)
        for _ in 0..<4 where row == nil {
            app.input.swipeUp()
            row = try advancedRow(app)
        }
        try app.tap(try XCTUnwrap(row, "Settings has no Advanced row"))
        for _ in 0..<6 where try !app.exists(save) {
            app.input.swipeUp()
            try app.settle()
        }
        try app.wait(address, "Settings → Advanced has no own endpoint form")
        return app
    }

    /// The Advanced row: on the iPhone hub a button whose label leads with the name; in the iPad's
    /// sidebar the name alone, as text.
    private func advancedRow(_ app: App) throws -> Element? {
        if ShowcaseApp.pad {
            return try app.query(.text("Advanced")).first
        }
        return try app.query(.textLike("Advanced").role("AXButton"))
            .first { $0.label.hasPrefix("Advanced") }
    }
}
