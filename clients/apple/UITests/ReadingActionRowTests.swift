// The actions over an open message (`docs/reading-actions.md`): on iPhone and iPad they are the
// navigation bar's, and the overflow reaches the system's print sheet.
//
// The bar's items are UIKit's, and UIKit names them for accessibility only in a running app, so a
// view hosted inside MailcalKit's suite sees five unlabelled buttons there. Their order and size are
// therefore held here. Which answer is the default is the core's rule, held by `mailcal-app`'s
// `a_message_defaults_to_reply_all_when_reply_all_reaches_someone_reply_does_not`; every showcase
// message is addressed to its reader alone, so here the default is Reply.

import XCTest

@MainActor
final class ReadingActionRowTests: XCTestCase {
    /// Archive and delete, then the default answer and the chevron holding the others, then the
    /// overflow last; all in the bar at the top.
    ///
    /// No width is asserted: a bar item reports its glyph's frame, 36 to 41 points, while the
    /// target a finger gets is the glass group's, which UIKit sizes as it does for every standard
    /// bar button and the app has no say in.
    func testTheBarHoldsTheMailboxActionsThenTheAnswerThenMore() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openFirstMessage(app)
        try app.wait(.button("Reply"), "no message was opened")

        let bar = ["Archive", "Delete", "Reply", "More ways to reply", "More actions"]
        let order = try ShowcaseApp.leftToRight(bar, in: app)
        XCTAssertEqual(Set(order.prefix(2)), ["Archive", "Delete"], "\(order)\n\(app.dump())")
        XCTAssertEqual(Array(order.suffix(3)), ["Reply", "More ways to reply", "More actions"])
        // The other answers are the chevron's, not the bar's.
        XCTAssertFalse(try app.exists(.button("Reply all")), "Reply all is on the bar")
        XCTAssertFalse(try app.exists(.button("Forward")), "Forward is on the bar")

        let height = app.input.windows.firstMatch.frame.height
        for label in bar {
            let frame = try XCTUnwrap(try app.wait(.button(label)).frame, "\(label) has no frame")
            XCTAssertLessThan(frame.maxY, height / 4, "\(label) is not in the top bar: \(frame)")
        }
    }

    /// Print, behind the overflow, reaches the system's print sheet.
    ///
    /// The page is laid out in a web view nobody sees, so the one thing a reader can observe is the
    /// sheet arriving; a web view that never finished loading leaves the press doing nothing.
    func testPrintOpensTheSystemPrintSheet() throws {
        let app = try ShowcaseApp.launch()
        try ShowcaseApp.openFirstMessage(app)

        try app.press(.button("More actions"), "no action row")
        // The body has to have arrived before there is anything to print.
        try app.wait(.button("Print…").enabled(true), "the menu offers no Print")
        // The sheet's own printer row, which no screen of ours draws.
        let printer = Filter.textLike("Printer")
        XCTAssertFalse(try app.exists(printer), "something on screen already reads as the print sheet")
        try app.press(.button("Print…"))

        try app.wait(printer, "no print sheet appeared")
    }
}
