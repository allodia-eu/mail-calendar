// Return and leaving the field finish an address as a comma does (docs/contacts.md): a phone's email
// keyboard has no comma on its first page. The keys are real ones, since what the keyboard sends is
// the question.

import XCTest

@MainActor
final class RecipientFinishTests: XCTestCase {
    private let to = Filter.text("To").role("AXTextField")

    func testReturnFinishesTheAddressAndKeepsTheField() throws {
        let app = try openComposer()
        // The composer opens with the caret in To.
        type("bob@example.test\n", in: app)
        try app.wait(pill("bob@example.test"), timeout: 5, "no pill")
        XCTAssertEqual(try app.wait(to).value, "", "the address stayed text")

        type("carol@example.test\n", in: app)
        try app.wait(pill("carol@example.test"), timeout: 5, "Return took the keyboard out of the field")
    }

    func testLeavingTheFieldFinishesTheAddress() throws {
        let app = try openComposer()
        type("bob@example.test", in: app)
        try app.tap(app.wait(.text("Subject").role("AXTextField")))
        try app.wait(pill("bob@example.test"), timeout: 5, "no pill")
    }

    private func openComposer() throws -> App {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        try app.press(.button("New Mail"))
        try app.wait(Filter().type("WKWebView"), "the composer drew no editor")
        return app
    }

    /// One key at a time, as `ComposerUndoTests` types into To, and for its reason.
    private func type(_ text: String, in app: App) {
        for letter in text {
            app.input.typeText(String(letter))
        }
    }

    /// A finished address's remove control, which names it.
    private func pill(_ address: String) -> Filter {
        .button("\(address), Remove recipient")
    }
}
