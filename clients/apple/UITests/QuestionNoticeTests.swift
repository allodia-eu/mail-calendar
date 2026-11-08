// The core's questions on iPhone (docs/notices.md rule 8): each a notice with two answers, behind
// the row under the bar, and answered there rather than in a sheet of its own.
//
// MAILCAL_SHOWCASE_QUESTIONS has the core raise all three, so everything from the question to the
// button is real: a sent message whose copy is not in Sent and an invitation reply not passed on,
// both on the first account, and a changed time zone.

import XCTest

@MainActor
final class QuestionNoticeTests: XCTestCase {
    func testEachQuestionIsANoticeAnsweredByItsOwnButtons() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_QUESTIONS": "1"])
        ShowcaseApp.showMailbox(app)
        let notice = app.buttons["notice"]
        XCTAssertTrue(
            notice.waitForExistence(timeout: ShowcaseApp.timeout), "the mailbox shows no notice"
        )
        XCTAssertTrue(
            notice.label.contains("Quarterly figures"),
            "the mailbox's first notice is not the missing Sent copy: \(notice.label)"
        )

        ShowcaseApp.tap(notice)
        XCTAssertTrue(
            app.buttons["Save to Sent"].waitForExistence(timeout: ShowcaseApp.timeout),
            "the copy's notice offers no Save to Sent"
        )
        XCTAssertTrue(app.buttons["Send the email"].exists, "the reply's notice offers no Send the email")
        XCTAssertTrue(app.buttons["Update"].exists, "the time zone's notice offers no Update")

        ShowcaseApp.tap(app.buttons["Not now"])
        XCTAssertTrue(
            app.buttons["Save to Sent"].waitForNonExistence(timeout: ShowcaseApp.timeout),
            "Not now left the copy's notice standing"
        )
        ShowcaseApp.tap(app.buttons["Don’t send"])
        XCTAssertTrue(
            app.buttons["Send the email"].waitForNonExistence(timeout: ShowcaseApp.timeout),
            "Don't send left the reply's notice standing"
        )
        ShowcaseApp.tap(app.buttons["Keep current"])
        XCTAssertTrue(
            app.buttons["Update"].waitForNonExistence(timeout: ShowcaseApp.timeout),
            "Keep current left the time zone's notice standing"
        )
    }
}
