// The core's notices on iPhone (docs/notices.md): one row under the bar of the tab on show, holding
// the first notice that tab has, and a sheet with each notice and its own button.
//
// The two reauth notices are staged by MAILCAL_SHOWCASE_REAUTH, which the core reads, so everything
// from the connectivity snapshot to the row is real. Mail's belongs to the mail tab and the
// calendar's to the calendar tab, so each tab shows its own.

import XCTest

@MainActor
final class NoticeTests: XCTestCase {
    func testEachTabShowsItsOwnReauthNoticeAndItsButton() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_REAUTH": "1"])
        ShowcaseApp.showMailbox(app)
        let notice = app.buttons["notice"]
        XCTAssertTrue(
            notice.waitForExistence(timeout: ShowcaseApp.timeout), "the mailbox shows no notice"
        )
        XCTAssertTrue(
            notice.label.contains("Sending and managing mail needs your permission"),
            "the mailbox's notice is not mail's: \(notice.label)"
        )

        ShowcaseApp.tap(notice)
        let signIn = app.buttons["Sign in again"]
        XCTAssertTrue(
            signIn.waitForExistence(timeout: ShowcaseApp.timeout), "the sheet offers no Sign in again"
        )
        ShowcaseApp.tap(app.buttons["Close"])
        XCTAssertTrue(signIn.waitForNonExistence(timeout: ShowcaseApp.timeout), "Close kept the sheet")

        ShowcaseApp.tap(app.buttons["Calendar"])
        XCTAssertTrue(
            app.buttons["New Event"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the calendar did not come up"
        )
        let calendarNotice = app.buttons["notice"]
        XCTAssertTrue(
            calendarNotice.waitForExistence(timeout: ShowcaseApp.timeout),
            "the calendar shows no notice"
        )
        XCTAssertTrue(
            calendarNotice.label.contains("Calendar access needs your permission"),
            "the calendar's notice is not the calendar's: \(calendarNotice.label)"
        )
    }
}
