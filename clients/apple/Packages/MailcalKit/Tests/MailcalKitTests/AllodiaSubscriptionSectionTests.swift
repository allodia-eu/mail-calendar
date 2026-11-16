// How the subscription section words what the core decided, pinned as policy rather than as
// pixels. What the section shows (billers, renewal, grace, store pages, which writes exist) is the
// core's and tested there; the drawing stays hand-verified, being a Settings pane with no state a
// headless test can reach.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@MainActor
struct AllodiaSubscriptionSectionTests {
    /// A biller this build cannot name still appears, or the "charged twice" warning would name
    /// only the half it recognises.
    @Test func anUnknownBillerStillHasAName() {
        #expect(AllodiaBiller.unknown(label: "Paddle").displayName == "Paddle")
        #expect(AllodiaBiller.allodia.displayName == "Allodia")
        #expect(AllodiaBiller.google.displayName == "Google Play")
    }

    /// A store page's button names the store the way the biller line does.
    @Test func aStorePageIsNamedAsItsBillerIs() {
        #expect(AllodiaStore.apple.displayName == AllodiaBiller.apple.displayName)
        #expect(AllodiaStore.google.displayName == AllodiaBiller.google.displayName)
    }

    /// The App Store is asked for its offers only while the account is free: a paid account, a
    /// section still checking and one that could not be read draw none.
    @Test func theStoreSellsOnlyToAFreeAccount() {
        #expect(allodiaSellsHere(.loaded(view: view(entitled: false))))
        #expect(!allodiaSellsHere(.loaded(view: view(entitled: true))))
        #expect(!allodiaSellsHere(.checking))
        #expect(!allodiaSellsHere(.unavailable))
        #expect(!allodiaSellsHere(.needsReauth))
        #expect(!allodiaSellsHere(.hidden))
        #expect(!allodiaSellsHere(nil))
    }

    /// The period is on the button: the two buy buttons and the two switch buttons each say which
    /// period they are for.
    @Test func eachPeriodHasButtonsOfItsOwn() {
        let yearly = AllodiaPlan.yearly.buyLabel(price: "€24,99")
        #expect(yearly != AllodiaPlan.monthly.buyLabel(price: "€24,99"))
        #expect(yearly.contains("€24,99"))
        #expect(AllodiaPlan.yearly.switchLabel != AllodiaPlan.monthly.switchLabel)
    }

    /// Every ending says something, and no two say the same thing.
    @Test func everyOutcomeEarnsASentenceOfItsOwn() {
        let sentences = [
            allodiaOutcomeText(.cancelled(endDate: "2026-10-18T00:00:00Z")),
            allodiaOutcomeText(.restarted),
            allodiaOutcomeText(switched),
            allodiaOutcomeText(.failed(failure: .notSwitchable)),
        ]
        #expect(sentences.allSatisfy { !$0.isEmpty })
        #expect(Set(sentences).count == sentences.count)
    }

    /// A cancellation names the day access runs to, formatted rather than left as the wire shape.
    @Test func aCancellationSaysWhatIsKeptAndUntilWhen() {
        let note = allodiaOutcomeText(.cancelled(endDate: "2026-10-18T00:00:00Z"))
        #expect(!note.contains("2026-10-18T00:00:00Z"))
        #expect(note.contains("2026"))
    }

    /// A switch says what the next charge becomes, priced in the currency it came with.
    @Test func aSwitchSaysWhatTheNextChargeBecomes() {
        #expect(allodiaOutcomeText(switched).contains("24"))
        // A switch that came with no currency still produces a sentence rather than nothing.
        let bare = AllodiaSubscriptionOutcome.switched(
            nextPaymentDate: nil, amountInCents: 2499, currency: "")
        #expect(!allodiaOutcomeText(bare).isEmpty)
    }

    /// Each refusal is a different thing to say. One sentence reused would tell somebody mid-retry
    /// that they are already cancelled.
    @Test func noTwoRefusalsSayTheSameThing() {
        let failures: [AllodiaWriteFailure] = [
            .unexplained, .alreadyCancelled, .alreadyActive, .alreadyOnInterval, .notSwitchable,
            .notFound,
        ]
        #expect(Set(failures.map(allodiaRefusalText)).count == failures.count)
        #expect(allodiaOutcomeText(.failed(failure: .notFound)) == allodiaRefusalText(.notFound))
    }

    /// A date is shown to the day: a renewal is what a bank statement will agree with, and an hour
    /// invites a comparison with a clock that means nothing here.
    ///
    /// ⚠️ **Every shape here is one the account service actually sends**, the offset form among
    /// them. A formatted answer is one that no longer looks like the wire, which is what these
    /// assert.
    @Test func aPeriodEndIsFormattedWhateverShapeItArrivesIn() {
        for raw in [
            "2026-10-18T09:30:00Z",
            "2026-10-18T09:30:00+00:00",
            "2026-10-18T11:30:00+02:00",
            "2026-10-18T09:30:00.123456Z",
            "2026-10-18",
        ] {
            let shown = allodiaDate(raw)
            #expect(shown != nil, "\(raw) produced nothing")
            #expect(shown != raw, "\(raw) was drawn as the wire rather than formatted")
            #expect(shown?.contains("2026") == true, "\(raw) lost its year")
        }
    }

    /// Nothing to say is said as nothing, rather than as an empty line where a date belongs.
    @Test func anAbsentPeriodEndIsNoLineAtAll() {
        #expect(allodiaDate("") == nil)
    }

    /// A **time** this build cannot read still yields a localised day, because the day is the only
    /// part this screen draws: the leading ten characters parse even when the rest does not.
    @Test func anUnreadableTimeStillYieldsALocalisedDay() {
        let shown = allodiaDate("2026-10-18 09:30 CET")
        #expect(shown != nil)
        #expect(shown != "2026-10-18", "the day parsed, so it should have been formatted")
        #expect(shown?.contains("2026") == true)
    }

    /// Something that is not a date at all is drawn as what arrived rather than as a guess. It is
    /// wrong either way; showing the wire at least says so.
    @Test func somethingThatIsNoDateIsNotInvented() {
        #expect(allodiaDate("no-such-date") == "no-such-da")
    }

    // MARK: builders

    private let switched = AllodiaSubscriptionOutcome.switched(
        nextPaymentDate: "2026-10-18T00:00:00Z", amountInCents: 2499, currency: "EUR")

    private func view(entitled: Bool) -> AllodiaSubscriptionView {
        AllodiaSubscriptionView(
            entitled: entitled,
            billedBy: entitled ? .allodia : nil,
            periodEnd: entitled ? "2026-10-18T00:00:00Z" : nil,
            renews: entitled,
            grace: false,
            duplicateBilling: [],
            storePages: [],
            paidThrough: entitled ? "2026-10-18T00:00:00Z" : nil,
            switch: nil,
            resubscribe: nil,
            cancel: entitled ? ControlState(enabled: true, running: false) : nil,
            checkout: nil,
            refreshing: false
        )
    }
}
