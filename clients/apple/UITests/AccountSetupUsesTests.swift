// What the found card offers an account for (`docs/account-autodetect.md`, rule 8): mail,
// calendar and contacts as the core decided them, the last use held on, and the card for a domain
// with a calendar and address book and no mail server.
//
// Which card appears is the core's detection, which the showcase build answers from a script keyed
// on the domain, so each card is reached here the way a person reaches it. What the toggles make of
// the account is `SetupUses`', tested in the package suite; this checks the screen draws it.

import XCTest

@MainActor
final class AccountSetupUsesTests: XCTestCase {
    /// The trusted domain's card: mail with its servers, a calendar and contacts found beside it.
    func testTheFoundCardOffersEachUseAndHoldsTheLastOneOn() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-detected"])
        let mail = app.switches["Use this account for mail"]
        XCTAssertTrue(mail.waitForExistence(timeout: ShowcaseApp.timeout), "no mail choice on the card")
        let calendar = app.switches["Also sync your calendar"]
        let contacts = app.switches["Also sync your contacts"]
        XCTAssertTrue(app.staticTexts["IMAP"].exists, "the mail servers are not shown while mail is on")

        tapSwitch(mail)
        XCTAssertFalse(app.staticTexts["IMAP"].exists, "the mail servers stayed after mail was switched off")
        tapSwitch(calendar)
        // Contacts are all that is left, and an account used for nothing is not one.
        XCTAssertFalse(contacts.isEnabled, "the last use could be switched off")
        XCTAssertEqual(contacts.value as? String, "1")
    }

    /// A domain with a calendar and address book and no mail server gets a card of its own.
    func testADomainWithoutAMailServerIsOfferedCalendarAndContactsAlone() {
        let app = ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        let field = app.textFields["you@example.com"]
        XCTAssertTrue(field.waitForExistence(timeout: ShowcaseApp.timeout))
        ShowcaseApp.tap(field)
        field.typeText("eva@cloud.example")
        ShowcaseApp.tap(app.buttons["Continue"])
        XCTAssertTrue(
            app.switches["Also sync your calendar"].waitForExistence(timeout: ShowcaseApp.timeout),
            "detection did not land on the calendar-and-contacts card"
        )
        XCTAssertTrue(app.switches["Also sync your contacts"].exists)
        XCTAssertFalse(
            app.switches["Use this account for mail"].exists,
            "a domain with no mail server was offered mail"
        )
    }

    /// A SwiftUI switch on iOS takes the tap on its control, at the trailing edge of the row.
    private func tapSwitch(_ toggle: XCUIElement) {
        toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
    }
}
