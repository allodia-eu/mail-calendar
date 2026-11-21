// The own AI endpoint's form under Settings → Advanced (`docs/ai.md` rules 4 to 7), in the running
// app. The showcase offers the writing assistant under `MAILCAL_OFFER_AI`, with an endpoint staged
// on this device and a host that sends nothing, so the form is there without an Allodia account.
// What a save refuses is asked of the core as the person types and Save is held meanwhile; what a
// save came to is said once. Without the hook Advanced draws no form at all.
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
    private let local = "http://localhost:11434/v1"

    func testWithoutTheOfferAdvancedHasNoEndpointForm() throws {
        let app = try SettingsCategories.open("Advanced")
        try SettingsCategories.reveal(.button("Reset Database"), in: app)
        XCTAssertFalse(try app.exists(address), "the endpoint's address is drawn while not offered")
        XCTAssertFalse(try app.exists(.text("Own AI endpoint")), "the form is drawn while not offered")
    }

    func testSaveIsHeldWhileAGapIsNamed() throws {
        let app = try openForm()
        try app.wait(save.enabled(true), "Save was held with the staged endpoint")

        try app.fill(model, "")
        try app.wait(.text("Enter the name of the model to use."), "the missing model was not said")
        try app.wait(save.enabled(false), "Save was not held without a model")
        try app.fill(model, "mistral")
        try app.wait(save.enabled(true), "Save stayed held with nothing missing")

        try app.fill(address, "http://ai.test.local/v1")
        try app.wait(
            .text("Use an https address, unless the server runs on this computer."),
            "an address elsewhere without HTTPS was not refused beside its field")
        try app.wait(save.enabled(false), "Save was not held for an address needing HTTPS")
    }

    func testASaveRunsToItsOutcomeAndSaysItOnce() throws {
        let app = try openForm()

        // The keystore refuses a key here: the form says so and keeps the key as typed.
        try app.fill(key, "sk-test")
        try app.press(save.enabled(true), "Save could not be pressed")
        try app.wait(.text("The key could not be stored."), "a refused key was not said")
        XCTAssertFalse(
            (try app.wait(key).value ?? "").isEmpty, "the refused key was taken out of its field")

        // The next change takes the line away, and a save without a key is kept. The refusal
        // before it is not said again.
        try app.fill(key, "")
        try app.fill(model, "mistral")
        try app.press(save.enabled(true))
        try app.wait(remove.enabled(true), "a kept endpoint offered no Remove")
        try app.settle()
        XCTAssertFalse(
            try app.exists(.text("The key could not be stored.")), "an outcome was said twice")

        try app.press(remove.enabled(true))
        try app.waitGone(remove, timeout: ShowcaseApp.timeout, "Remove stayed after the removal")
        XCTAssertEqual(try app.wait(address).value ?? "", "", "the removed address is still drawn")
        try app.wait(save.enabled(false), "Save was free with every field empty")
    }

    /// Settings → Advanced, scrolled to the form, which opens on the staged endpoint.
    private func openForm() throws -> App {
        let app = try SettingsCategories.open("Advanced", hooks: ["MAILCAL_OFFER_AI": "1"])
        try SettingsCategories.reveal(save, in: app)
        XCTAssertEqual(try app.wait(address).value, local, "the staged endpoint is not drawn")
        return app
    }
}
