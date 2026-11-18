// Undo and redo while writing on iPhone and iPad (docs/undo.md).
//
// What a step is, the bundle's tests hold for the body (`history.test.ts`) and the kit suite for a
// recipient field (`RecipientHistoryTests`). Neither can see whether the platform's chords reach
// those histories rather than WebKit's or UIKit's own, which is what this suite is for: WebKit's
// undo of a typed run differs from the bundle's, and UIKit's undo in To types a finished address
// back in. So the keys and chords here are real ones, and the message's text, which is the web
// view's and out of the agent's reach, is read through XCUITest.
//
// ⚠️ The iPad's shortcut bar, shaking and the three-finger gestures are verified by hand
// (docs/undo.md). None can be held here: the first ⌘Z a test sends tells the simulator a keyboard
// is attached, which takes the on-screen keyboard and its shortcut bar away for the rest of the
// run, the first Undo on a fresh simulator raises a system tip over the keyboard, and XCUITest has
// no way to shake or to swipe with three fingers.

import XCTest

@MainActor
final class ComposerUndoTests: XCTestCase {
    func testTheChordsStepTheBodysHistory() throws {
        let (app, web) = try openComposer()
        try typeTwoRuns(into: web, in: app)

        XCTAssertTrue(
            try app.chord("z", .command) { !text("beta", in: web).exists },
            "⌘Z did not take back the second run"
        )
        XCTAssertTrue(text("alpha", in: web).exists, "⌘Z took back more than the second run")

        app.input.typeKey("z", modifierFlags: [.command, .shift])
        XCTAssertTrue(
            text("alpha beta", in: web).waitForExistence(timeout: 5), "⇧⌘Z did not put the run back"
        )
    }

    func testAFinishedRecipientUndoesToTextNotASecondPill() throws {
        let (app, _) = try openComposer()
        // The composer opens with the caret in To.
        type("bob@example.test,", in: app)
        try app.wait(pill("bob@example.test"), timeout: 5, "no pill")

        XCTAssertTrue(
            try app.chord("z", .command) { try !app.exists(pill("bob@example.test")) },
            "⌘Z left the address a pill"
        )
        XCTAssertEqual(try app.wait(to).value, "bob@example.test")

        app.input.typeKey("z", modifierFlags: [.command, .shift])
        try app.wait(pill("bob@example.test"), timeout: 5, "⇧⌘Z lost it")
        XCTAssertEqual(
            try app.query(.textLike(", Remove recipient")).count, 1, "the address was finished twice"
        )
    }

    func testARemovedRecipientComesBack() throws {
        let (app, _) = try openComposer()
        type("bob@example.test,", in: app)
        try app.press(pill("bob@example.test"), timeout: 5, "no pill")
        try app.waitGone(pill("bob@example.test"), "the pill was not removed")

        // Removing it put the caret back in the field, where the chord is heard.
        XCTAssertTrue(
            try app.chord("z", .command) { try app.exists(pill("bob@example.test")) },
            "⌘Z did not bring the address back"
        )
    }

    private let to = Filter.text("To").role("AXTextField")

    private func openComposer() throws -> (App, XCUIElement) {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        ShowcaseApp.attachHardwareKeyboard(app)
        try app.press(.button("New Mail"))
        let web = app.input.webViews.firstMatch
        // The message's own text view, not the web view: the view exists before its page has
        // loaded, and a tap on a page with no editor in it focuses nothing.
        XCTAssertTrue(
            web.textViews.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout),
            "the composer drew no editor"
        )
        return (app, web)
    }

    /// Two runs of typing in the body, a pause apart, so each is a step of its own
    /// (`TYPING_PAUSE_MS`).
    private func typeTwoRuns(into web: XCUIElement, in app: App) throws {
        try ShowcaseApp.composerBody(app).tap()
        app.input.typeText("alpha")
        XCTAssertTrue(text("alpha", in: web).waitForExistence(timeout: 5), "nothing was typed")
        Thread.sleep(forTimeInterval: 1.5)
        app.input.typeText(" beta")
        XCTAssertTrue(text("alpha beta", in: web).waitForExistence(timeout: 5), "nothing was typed")
    }

    /// One key at a time, so the app settles between them: typed in one call, a key that lands
    /// while a pill or the suggestion list moves the header is lost. Never in the body, where a
    /// pause between keys is the end of a step (`TYPING_PAUSE_MS`).
    private func type(_ text: String, in app: App) {
        for letter in text {
            app.input.typeText(String(letter))
        }
    }

    /// Text in the message, which only XCUITest can read.
    private func text(_ words: String, in web: XCUIElement) -> XCUIElement {
        web.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", words)).firstMatch
    }

    /// A finished address's remove control, which names it.
    private func pill(_ address: String) -> Filter {
        .button("\(address), Remove recipient")
    }
}
