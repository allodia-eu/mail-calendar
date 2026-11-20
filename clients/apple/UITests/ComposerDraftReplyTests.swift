// Draft a reply in the composer (`docs/ai.md` rules 17 to 24), on the showcase with
// `MAILCAL_OFFER_AI`. Its seed lets the first account draft in "Work" and stages a canned answer:
// a reply with one gap, a file to attach and a task to do. The second account drafts in no style,
// so its reply holds Draft with a remedy. The canned answer comes from inside the app, and a reply
// sent from the showcase reaches nobody: nothing leaves the machine.
//
// The message's text is the web view's, out of the agent's reach, so it is read through XCUITest;
// undo and redo are real chords, for the reason `ComposerUndoTests` gives.

import XCTest

@MainActor
final class ComposerDraftReplyTests: XCTestCase {
    private let control = Filter.id("composer.draftReply").role("AXButton")
    private let instruction = Filter.id("composer.draftInstruction")
    private let card = Filter.id("composer.draftCard")
    private let drafted = "We can deliver on"

    func testTheControlOpensItsInputAndASuggestionFillsTheLine() throws {
        let (app, _) = try reply(to: "Welcome to Allodia Mail & Calendar")
        try app.press(control, "the reply has no Draft a reply")
        try app.wait(instruction, "the control did not open the instruction's input")
        try app.press(.button("Yes"), "the input offers no suggestions")
        XCTAssertEqual(try app.wait(instruction).value, "Yes", "the suggestion did not fill the line")
        try app.wait(.button("Draft").enabled(true), "Draft was held")
    }

    func testAnAccountWithoutAStyleSaysWhyWithItsRemedy() throws {
        let (app, _) = try reply(to: "Welcome to your first week")
        try app.press(control, "the reply has no Draft a reply")
        try app.wait(
            .text("No writing style is chosen for this account."), "the held draft did not say why")
        try app.wait(.button("Choose a style").enabled(true), "the held draft offered no remedy")
        XCTAssertFalse(try app.exists(instruction), "the field was drawn while Draft is held")
    }

    func testADraftGoesInAsOneStepAndTheCardFollowsIt() throws {
        let (app, web) = try reply(to: "Welcome to Allodia Mail & Calendar")
        try draft(in: app)
        try app.wait(card, timeout: 30, "the draft brought no card")
        try app.wait(.textLike("They ask whether the order can be delivered"), "no summary")
        try app.wait(.textLike("Book the delivery van"), "the checklist has no task")
        XCTAssertTrue(text(drafted, in: web).waitForExistence(timeout: 5), "no draft in the reply")

        try ShowcaseApp.composerBody(app).tap()
        XCTAssertTrue(
            try app.chord("z", .command) { !text(drafted, in: web).exists },
            "one undo did not take the draft out")
        try app.waitGone(card, timeout: 10, "the card stayed while the draft was undone")
        app.input.typeKey("z", modifierFlags: [.command, .shift])
        XCTAssertTrue(text(drafted, in: web).waitForExistence(timeout: 5), "redo did not put it back")
        try app.wait(card, timeout: 10, "the card did not come back with the draft")
    }

    func testTheReplaceQuestionKeepsWhatWasWrittenOnCancel() throws {
        let (app, web) = try reply(to: "Welcome to Allodia Mail & Calendar")
        try ShowcaseApp.composerBody(app).tap()
        // The reply's body takes the caret as the page settles; a key sent before then is lost.
        try app.settle()
        app.input.typeText("Mine")
        XCTAssertTrue(text("Mine", in: web).waitForExistence(timeout: 5), "nothing was typed")
        try app.press(control)
        try app.press(.button("Draft"))
        try app.wait(.text("Replace what you have written?"), "Draft did not ask first")
        try app.press(.button("Cancel"))
        XCTAssertTrue(text("Mine", in: web).exists, "Cancel took the person's text")
        XCTAssertFalse(try app.exists(card), "Cancel still drafted")
        try app.press(control)
        try app.press(.button("Draft"))
        try app.press(.button("Replace"))
        try app.wait(card, timeout: 30, "Replace brought no draft")
    }

    func testATickAndSendAskingOnceThenSending() throws {
        let (app, _) = try reply(to: "Welcome to Allodia Mail & Calendar")
        try draft(in: app)
        try app.wait(card, timeout: 30, "the draft brought no card")
        let task = try app.wait(.textLike("Book the delivery van"), "no task")
        XCTAssertFalse(ticked(task), "the task opened ticked")
        try app.activate(task)
        XCTAssertTrue(
            try wait(until: { ticked(try app.wait(.textLike("Book the delivery van"))) }),
            "the tick did not take")

        try app.press(.button("Send"))
        try app.wait(.text("Send with open items?"), "Send did not ask with items open")
        try app.press(.button("Keep editing"))
        try app.wait(card, "Keep editing left the composer")
        try app.press(.button("Send"))
        try app.waitGone(card, timeout: ShowcaseApp.timeout, "Send asked again or did not send")
        XCTAssertFalse(try app.exists(.text("Send with open items?")), "Send asked twice")
    }

    func testSendAnywaySends() throws {
        let (app, _) = try reply(to: "Welcome to Allodia Mail & Calendar")
        try draft(in: app)
        try app.wait(card, timeout: 30, "the draft brought no card")
        try app.press(.button("Send"))
        try app.press(.button("Send anyway"), "Send did not ask with items open")
        try app.waitGone(card, timeout: ShowcaseApp.timeout, "Send anyway did not send")
    }

    /// A reply to the message whose row mentions `subject`, with the hook's offer on.
    private func reply(to subject: String) throws -> (App, XCUIElement) {
        let app = try ShowcaseApp.launch(["MAILCAL_OFFER_AI": "1"])
        ShowcaseApp.attachHardwareKeyboard(app)
        try ShowcaseApp.openMessage(containing: subject, in: app)
        try app.press(.button("Reply"), "the message has no Reply")
        let web = app.input.webViews.firstMatch
        XCTAssertTrue(
            web.textViews.firstMatch.waitForExistence(timeout: ShowcaseApp.timeout),
            "the composer drew no editor")
        try app.wait(control, "the reply has no Draft a reply")
        return (app, web)
    }

    /// Draft with an empty line, answering Replace if the editor has not yet reported an empty
    /// body: the record asks until it has.
    private func draft(in app: App) throws {
        try app.press(control)
        try app.press(.button("Draft").enabled(true), "Draft was held")
        if try app.appears(.button("Replace"), timeout: 2) {
            try app.press(.button("Replace"))
        }
    }

    /// Whether a checklist row reads as ticked: selected on iOS, a value of 1 on a Mac's box.
    private func ticked(_ row: Element) -> Bool {
        row.selected || row.value == "1"
    }

    private func wait(until condition: () throws -> Bool) throws -> Bool {
        let deadline = Date().addingTimeInterval(5)
        repeat {
            if try condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        } while Date() < deadline
        return false
    }

    /// Text in the message, which only XCUITest can read.
    private func text(_ words: String, in web: XCUIElement) -> XCUIElement {
        web.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", words)).firstMatch
    }
}
