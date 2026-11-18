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
    private let mail = Filter.text("Use this account for mail").role("AXSwitch")
    private let calendar = Filter.text("Also sync your calendar").role("AXSwitch")
    private let contacts = Filter.text("Also sync your contacts").role("AXSwitch")

    /// The trusted domain's card: mail with its servers, a calendar and contacts found beside it.
    func testTheFoundCardOffersEachUseAndHoldsTheLastOneOn() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "setup-detected"])
        let mailSwitch = try app.wait(mail, "no mail choice on the card")
        XCTAssertTrue(
            try app.exists(.text("IMAP")), "the mail servers are not shown while mail is on"
        )

        try app.toggle(mailSwitch, to: false)
        XCTAssertTrue(
            try app.goes(.text("IMAP")), "the mail servers stayed after mail was switched off"
        )
        try app.toggle(try app.wait(calendar), to: false)
        // Contacts are all that is left, and an account used for nothing is not one.
        let held = try app.wait(contacts)
        XCTAssertFalse(held.enabled, "the last use could be switched off")
        XCTAssertEqual(held.active, true)
    }

    /// A domain with a calendar and address book and no mail server gets a card of its own.
    func testADomainWithoutAMailServerIsOfferedCalendarAndContactsAlone() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "add-account"])
        try app.fill(.text("you@example.com").role("AXTextField"), "eva@cloud.example")
        try app.press(.button("Continue"))
        try app.wait(calendar, "detection did not land on the calendar-and-contacts card")
        XCTAssertTrue(try app.exists(contacts))
        XCTAssertFalse(try app.exists(mail), "a domain with no mail server was offered mail")
        // Detection found it, so the card says there is no mail server, and, as on the mail card,
        // that the server may want an app password.
        XCTAssertTrue(
            try startsWith("We found a calendar and address book server", in: app),
            "the card does not say what detection found"
        )
        XCTAssertTrue(
            try startsWith("Some providers require an app-specific password", in: app),
            "the card does not mention app passwords"
        )
    }

    /// Whether a text on screen starts with `prefix`.
    private func startsWith(_ prefix: String, in app: App) throws -> Bool {
        try app.query(.textLike(prefix).role("AXStaticText")).contains { $0.label.hasPrefix(prefix) }
    }
}
