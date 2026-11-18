// A conversation row's menu and how a conversation expands (`docs/row-actions.md`), on the
// showcase's one conversation: Tom's request, the owner's reply filed in Sent, and Tom's unread
// follow-up, which is the newest message.
//
// A unit test sees what the core answers and what the client would draw from it; only this sees
// what a long press actually raises, and which rows the list holds once the user moves on.

import XCTest

@MainActor
final class RowMenuTests: XCTestCase {
    /// The conversation's subject, shown on its row only.
    private let conversation = "Q3 launch"
    /// The owner's reply's preview, shown only once the conversation is expanded.
    private let reply = "Checklist looks solid"

    /// A conversation offers what one of its messages does: the newest is unread and received,
    /// so it can be answered, marked read, flagged and reported, and the conversation archived.
    func testAConversationOffersWhatAMessageDoes() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.showMailbox(app)
        // A finger held on the row: the menu is what a long press raises.
        try app.longPress(app.wait(ShowcaseApp.row(mentioning: conversation)))

        for label in ["Reply", "Reply all", "Forward", "Mark as read", "Flag", "Mark as Spam",
                      "Archive", "Delete"] {
            XCTAssertTrue(
                try app.appears(.button(label), timeout: 5),
                "the conversation's menu has no '\(label)'"
            )
        }
        XCTAssertTrue(
            try app.wait(.button("Reply")).enabled, "the newest message was received"
        )
        XCTAssertFalse(
            try app.exists(.button("Archive conversation")),
            "the conversation's archive is the message menu's Archive"
        )
    }

    /// Opening the conversation lays its messages out in the list; opening any other row folds
    /// them away again, so a conversation the user has left does not stay open behind them.
    func testLeavingAConversationFoldsItAway() throws {
        try XCTSkipIf(
            ShowcaseApp.pad, "the iPad reads beside the list, with no Back"
        )
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openMessage(containing: conversation, in: app)
        try ShowcaseApp.back(app)
        try app.wait(
            ShowcaseApp.row(mentioning: reply), "opening the conversation did not lay out its messages"
        )

        try ShowcaseApp.openMessage(containing: "Your June usage report", in: app)
        try ShowcaseApp.back(app)
        try app.waitGone(
            ShowcaseApp.row(mentioning: reply), "the conversation stayed open after the user left it"
        )
    }
}
