// The core's questions on iPhone (docs/notices.md rule 8): each a notice with two answers, behind
// the row under the bar, and answered there rather than in a sheet of its own.
//
// MAILCAL_SHOWCASE_QUESTIONS has the core raise all three, so everything from the question to the
// button is real: a sent message whose copy is not in Sent and an invitation reply not passed on,
// both on the first account, and a changed time zone.

import XCTest

@MainActor
final class QuestionNoticeTests: XCTestCase {
    func testEachQuestionIsANoticeAnsweredByItsOwnButtons() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_QUESTIONS": "1"])
        try ShowcaseApp.showMailbox(app)
        let notice = try app.wait(.id("notice"), "the mailbox shows no notice")
        XCTAssertTrue(
            notice.label.contains("Quarterly figures"),
            "the mailbox's first notice is not the missing Sent copy: \(notice.label)"
        )

        try app.activate(notice)
        try app.wait(.button("Save to Sent"), "the copy's notice offers no Save to Sent")
        XCTAssertTrue(
            try app.exists(.button("Send the email")), "the reply's notice offers no Send the email"
        )

        // The sheet opens at half height, so the third notice is drawn once the first two are
        // answered and it moves up.
        try app.press(.button("Not now"))
        try app.waitGone(
            .button("Save to Sent"), timeout: ShowcaseApp.timeout,
            "Not now left the copy's notice standing"
        )
        try app.press(.button("Don’t send"))
        try app.waitGone(
            .button("Send the email"), timeout: ShowcaseApp.timeout,
            "Don't send left the reply's notice standing"
        )
        try app.wait(.button("Update"), "the time zone's notice offers no Update")
        try app.press(.button("Keep current"))
        try app.waitGone(
            .button("Update"), timeout: ShowcaseApp.timeout,
            "Keep current left the time zone's notice standing"
        )
    }
}
