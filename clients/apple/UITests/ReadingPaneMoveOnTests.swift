// Where the reading pane goes after an archive from it (`docs/reading-actions.md`, "Where the pane
// goes next"): the message the core names, opened as clicking its row would.
//
// Which message that is is the core's, pinned in `reading_stop_tests.rs`; what only this can see is
// that the client asks it and opens the answer where a click would have. In the showcase the
// invitation sits above the launch conversation, whose newest message is Tom's follow-up: a click
// on that row expands the conversation and opens the follow-up, so the pane moving on has to do
// both.

import XCTest

@MainActor
final class ReadingPaneMoveOnTests: XCTestCase {
    /// The archived message, directly above the conversation.
    private let archived = "Partnership kickoff"
    /// The conversation's subject, on its row.
    private let conversation = "Q3 launch"
    /// The owner's reply's preview, a row of its own only while the conversation is expanded.
    private let reply = "Checklist looks solid"
    /// Words only in the body of Tom's follow-up, the conversation's newest message.
    private let newestBody = "update the runbook"

    func testArchivingMovesThePaneOnAsAClickOnTheNextRowWould() throws {
        try XCTSkipIf(
            !ShowcaseApp.pad, "a phone goes back to the list instead"
        )
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openMessage(containing: archived, in: app)
        try app.wait(.button("Reply"), "the message to archive did not open")
        try app.activate(readingArchive(app))

        // The body is web content, which only XCUITest sees.
        let body = app.input.webViews.staticTexts
            .containing(NSPredicate(format: "label CONTAINS[c] %@", newestBody)).firstMatch
        XCTAssertTrue(
            body.waitForExistence(timeout: ShowcaseApp.timeout),
            "the pane did not move on to the newest message of '\(conversation)'"
        )
        XCTAssertTrue(
            try app.appears(ShowcaseApp.row(mentioning: reply), timeout: 5),
            "the pane opened a message of a collapsed conversation without expanding it, as a click on its row would"
        )
    }

    /// The reading view's Archive, rightmost of the buttons by that name: the sidebar's Archive
    /// folder shares the label.
    private func readingArchive(_ app: App) throws -> Element {
        let archives = try app.waitAll(.button("Archive"), "no Archive on screen")
        return archives.max { ($0.frame?.midX ?? 0) < ($1.frame?.midX ?? 0) } ?? archives[0]
    }
}
