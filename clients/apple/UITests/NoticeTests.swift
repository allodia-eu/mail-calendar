// The core's notices on iPhone (docs/notices.md): one row under the bar of the tab on show, holding
// the first notice that tab has, and a sheet with each notice and its own button.
//
// The two reauth notices are staged by MAILCAL_SHOWCASE_REAUTH, which the core reads, so everything
// from the core's facts to the row is real. Mail's belongs to the mail tab and the
// calendar's to the calendar tab, so each tab shows its own.

import XCTest

@MainActor
final class NoticeTests: XCTestCase {
    func testEachTabShowsItsOwnReauthNoticeAndItsButton() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_REAUTH": "1"])
        try ShowcaseApp.showMailbox(app)
        let notice = try app.wait(.id("notice"), "the mailbox shows no notice")
        XCTAssertTrue(
            notice.label.contains("Sending and managing mail needs your permission"),
            "the mailbox's notice is not mail's: \(notice.label)"
        )

        try app.activate(notice)
        let signIn = Filter.button("Sign in again")
        try app.wait(signIn, "the sheet offers no Sign in again")
        try app.press(.button("Close"))
        try app.waitGone(signIn, timeout: ShowcaseApp.timeout, "Close kept the sheet")

        try app.press(.button("Calendar"))
        try app.wait(.button("New Event"), "the calendar did not come up")
        let calendarNotice = try app.wait(.id("notice"), "the calendar shows no notice")
        XCTAssertTrue(
            calendarNotice.label.contains("Calendar access needs your permission"),
            "the calendar's notice is not the calendar's: \(calendarNotice.label)"
        )
    }
}
