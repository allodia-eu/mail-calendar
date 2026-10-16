// Undo and redo while writing on iPhone and iPad (docs/undo.md).
//
// What a step is, the bundle's tests hold for the body (`history.test.ts`) and the kit suite for a
// recipient field (`RecipientHistoryTests`). Neither can see whether the platform's chords reach
// those histories rather than WebKit's or UIKit's own, which is what this suite is for: WebKit's
// undo of a typed run differs from the bundle's, and UIKit's undo in To types a finished address
// back in.
//
// ⚠️ The iPad's shortcut bar and shaking are verified by hand (docs/undo.md). Neither can be held
// here: the first ⌘Z a test sends tells the simulator a keyboard is attached, which takes the
// on-screen keyboard and its shortcut bar away for the rest of the run, the first Undo on a fresh
// simulator raises a system tip over the keyboard, and XCUITest has no way to shake.

import XCTest

@MainActor
final class ComposerUndoTests: XCTestCase {
    func testTheChordsStepTheBodysHistory() {
        let (app, web) = openComposer()
        typeTwoRuns(into: web, in: app)

        app.typeKey("z", modifierFlags: .command)
        XCTAssertTrue(gone(text("beta", in: web)), "⌘Z did not take back the second run")
        XCTAssertTrue(text("alpha", in: web).exists, "⌘Z took back more than the second run")

        app.typeKey("z", modifierFlags: [.command, .shift])
        XCTAssertTrue(
            text("alpha beta", in: web).waitForExistence(timeout: 5), "⇧⌘Z did not put the run back"
        )
    }

    func testAFinishedRecipientUndoesToTextNotASecondPill() {
        let (app, _) = openComposer()
        // The composer opens with the caret in To.
        type("bob@example.test,", in: app)
        XCTAssertTrue(pill("bob@example.test", in: app).waitForExistence(timeout: 5), "no pill")

        app.typeKey("z", modifierFlags: .command)
        XCTAssertTrue(gone(pill("bob@example.test", in: app)), "⌘Z left the address a pill")
        XCTAssertEqual(app.textFields["To"].value as? String, "bob@example.test")

        app.typeKey("z", modifierFlags: [.command, .shift])
        XCTAssertTrue(pill("bob@example.test", in: app).waitForExistence(timeout: 5), "⇧⌘Z lost it")
        XCTAssertEqual(pills(in: app).count, 1, "the address was finished twice")
    }

    func testARemovedRecipientComesBack() {
        let (app, _) = openComposer()
        type("bob@example.test,", in: app)
        let bob = pill("bob@example.test", in: app)
        XCTAssertTrue(bob.waitForExistence(timeout: 5), "no pill")
        ShowcaseApp.tap(bob)
        XCTAssertTrue(gone(bob), "the pill was not removed")

        // Removing it put the caret back in the field, where the chord is heard.
        app.typeKey("z", modifierFlags: .command)
        XCTAssertTrue(bob.waitForExistence(timeout: 5), "⌘Z did not bring the address back")
    }

    private func openComposer() -> (XCUIApplication, XCUIElement) {
        let app = ShowcaseApp.launch()
        ShowcaseApp.showMailbox(app)
        ShowcaseApp.tap(app.buttons["New Mail"])
        let web = app.webViews.firstMatch
        XCTAssertTrue(web.waitForExistence(timeout: ShowcaseApp.timeout), "the composer drew no editor")
        return (app, web)
    }

    /// Two runs of typing in the body, a pause apart, so each is a step of its own
    /// (`TYPING_PAUSE_MS`).
    private func typeTwoRuns(into web: XCUIElement, in app: XCUIApplication) {
        web.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.55)).tap()
        app.typeText("alpha")
        XCTAssertTrue(text("alpha", in: web).waitForExistence(timeout: 5), "nothing was typed")
        Thread.sleep(forTimeInterval: 1.5)
        app.typeText(" beta")
        XCTAssertTrue(text("alpha beta", in: web).waitForExistence(timeout: 5), "nothing was typed")
    }

    /// One key at a time, so the app settles between them: typed in one call, a key that lands
    /// while a pill or the suggestion list moves the header is lost. Never in the body, where a
    /// pause between keys is the end of a step (`TYPING_PAUSE_MS`).
    private func type(_ text: String, in app: XCUIApplication) {
        for letter in text {
            app.typeText(String(letter))
        }
    }

    private func text(_ words: String, in web: XCUIElement) -> XCUIElement {
        web.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", words)).firstMatch
    }

    /// A finished address's remove control, which names it.
    private func pill(_ address: String, in app: XCUIApplication) -> XCUIElement {
        app.buttons["\(address), Remove recipient"]
    }

    private func pills(in app: XCUIApplication) -> [XCUIElement] {
        app.buttons.matching(NSPredicate(format: "label ENDSWITH %@", ", Remove recipient"))
            .allElementsBoundByIndex
    }

    private func gone(_ element: XCUIElement) -> Bool {
        element.waitForNonExistence(timeout: 5)
    }
}
