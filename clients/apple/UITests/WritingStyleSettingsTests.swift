// Settings → Writing style in the running app (`docs/ai.md` rules 4 and 8 to 16), on the showcase
// with `MAILCAL_OFFER_AI`: its seed gives the first account two styles, "Work", already revealed,
// which the account drafts in, and "Newly learned", whose reveal waits, and stages an endpoint on
// this device, and sent mail on the first account that says enough to learn from. Learning there
// fails as unreachable, so a run is driven through Review to Failed.
// Nothing leaves the machine: the staged host answers from inside the app.

import UIKit
import XCTest

@MainActor
final class WritingStyleSettingsTests: XCTestCase {
    private let offer = ["MAILCAL_OFFER_AI": "1"]
    private let account = "eva.jansen@example.com"

    func testWithoutTheOfferThereIsNoCategory() throws {
        let app = try ShowcaseApp.launch(["MAILCAL_SHOWCASE_SCREEN": "settings"])
        try app.wait(.text("Settings").role("AXHeading"), "Settings did not open")
        try app.wait(.textLike("Signatures"), "Settings has no Signatures row")
        for _ in 0..<4 { app.input.swipeUp() }
        XCTAssertNil(
            try SettingsCategories.row("Writing style", in: app),
            "Writing style is listed while the assistant is not offered")
    }

    func testThePageListsTheStylesAndEachAccountsStyle() throws {
        let app = try SettingsCategories.open("Writing style", hooks: offer)
        try app.wait(.textLike("Work, Learned from"), "the drafting style has no row")
        try app.wait(.textLike("Newly learned, Learned from"), "the new style has no row")
        try app.wait(.id("writingStyle.learn").role("AXButton").enabled(true), "Learn was held")
        try SettingsCategories.reveal(.text("Style for each account"), in: app)
    }

    func testTheFlowGoesBackAndOnToTheReview() throws {
        let app = try SettingsCategories.open("Writing style", hooks: offer)
        try reachTheReview(app)
        try back(app, "the review has no back")
        try app.wait(.text("Which sent mail?"), "Back did not return to the range")
        try app.press(.button("Cancel"))
        try app.waitGone(.text("Which sent mail?"), timeout: ShowcaseApp.timeout, "Cancel left it")
    }

    func testARunThatCannotReachItsEndpointFailsWithItsRemedy() throws {
        let app = try SettingsCategories.open("Writing style", hooks: offer)
        try reachTheReview(app)
        try app.press(.button("Learn").enabled(true), timeout: 30, "the review never offered Learn")
        try app.wait(.text("No style was learned"), timeout: 30, "the run did not fail")
        try app.wait(.text("The AI service could not be reached."), "the failure was not said")
        try app.wait(.button("Try again"), "the failure has no remedy")
        try app.press(.button("Close"))
        try app.waitGone(.text("No style was learned"), timeout: ShowcaseApp.timeout, "Close left it")
    }

    /// The flow's back button: the Settings stack under the sheet has one of its own, higher up,
    /// since the sheet's bar sits below the top of the screen.
    private func back(_ app: App, _ because: String) throws {
        let buttons = try app.waitAll(.id("BackButton"), because)
        let flow = buttons.max { ($0.frame?.minY ?? 0) < ($1.frame?.minY ?? 0) }
        try app.activate(try XCTUnwrap(flow, because))
    }

    /// Learn, an account, Back to the accounts and on again, the range, and Next to the review.
    private func reachTheReview(_ app: App) throws {
        try app.press(.id("writingStyle.learn").role("AXButton"), "Learn could not be pressed")

        try app.wait(.text("Learn from which account?"), "the flow did not open on the accounts")
        try app.wait(.button("Next").enabled(false), "Next was free with no account chosen")
        try app.press(.button(account))
        try app.press(.button("Next").enabled(true), "Next stayed held with an account chosen")

        try app.wait(.text("Which sent mail?"), "Next did not go on to the range")
        try back(app, "the range has no back")
        try app.wait(.text("Learn from which account?"), "Back did not return to the accounts")
        try app.press(.button("Next"))
        try app.wait(.text("Which sent mail?"))
        try app.wait(.button("The last twelve months"), "the range does not offer the last year")
        try app.wait(.button("All your sent mail"), "the range does not offer all sent mail")
        try app.wait(.button("Between two dates"), "the range of the person's own is not offered")
        try app.press(.button("Next"))

        try app.wait(.text("Before anything is sent"), "Next did not go on to the review")
        try app.wait(.textLike("are sent to localhost"), "the review does not say where it sends")
    }

    func testANewStyleOpensItsRevealAndSavingItOpensTheDetailAfter() throws {
        let app = try SettingsCategories.open("Writing style", hooks: offer)
        try app.press(.textLike("Newly learned, Learned from").role("AXButton"))
        try app.wait(.text("What we noticed about how you write"), "the row did not open the reveal")
        for _ in 0..<5 where try !app.exists(.id("writingStyle.reveal.name")) {
            app.input.swipeLeft()
            try app.settle()
        }
        try app.fill(.id("writingStyle.reveal.name"), "   ")
        try app.wait(.text("Give the style a name."), "a blank name was not refused")
        try app.wait(.button("Save").enabled(false), "Save was free with a blank name")
        try app.fill(.id("writingStyle.reveal.name"), "Weekdays")
        try app.press(.button("Save").enabled(true), "Save stayed held with a name")

        try app.wait(.textLike("Weekdays, Learned from"), "the saved name is not on its row")
        try app.press(.textLike("Weekdays, Learned from").role("AXButton"))
        try app.wait(.id("writingStyle.detail.name"), "the row opened the reveal again")
    }

    func testTheDetailAsksBeforeForgettingAndNamesTheAccount() throws {
        let app = try SettingsCategories.open("Writing style", hooks: offer)
        try app.press(.textLike("Work, Learned from").role("AXButton"))
        try app.wait(.id("writingStyle.detail.name"), "the row did not open the detail")
        let forget = try SettingsCategories.reveal(.id("writingStyle.detail.forget"), in: app)
        try app.activate(forget)
        try app.wait(.text("Forget “Work”?"), "Forget did not ask")
        try app.wait(.textLike(account), "the question does not name the account drafting in it")
        try app.press(.button("Cancel"))
        try app.wait(.id("writingStyle.detail.forget"), "Cancel forgot the style")
    }
}
