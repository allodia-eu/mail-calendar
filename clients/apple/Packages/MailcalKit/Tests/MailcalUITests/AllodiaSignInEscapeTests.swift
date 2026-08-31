// The way out of a sign-in that does not come back, on the one screen a person cannot skip
// (docs/onboarding.md). There is no Apple UI-test target, so what is pinned here is the half that
// is logic: the threshold the card waits before offering the way back, and the guarantee that an
// escaped attempt never reaches the browser.

import Foundation
import Testing

@testable import MailcalUI

@MainActor
struct AllodiaSignInEscapeTests {
    /// A hop that comes straight back must not put a button in front of somebody who had no reason
    /// to read it, and one that hangs must not leave them there. The contract caps the wait at ten
    /// seconds; eight is what Android and Linux hold, and holding a different one here would be a
    /// different app on every second platform.
    @Test func theWayBackIsOfferedWithinTheContractsBound() {
        #expect(OnboardingAllodiaCard.escapeAfter <= .seconds(10))
        #expect(OnboardingAllodiaCard.escapeAfter == .seconds(8))
    }

    /// Escaping is not a failure: it ends as a cancellation, which the card says nothing about.
    ///
    /// What this pins is the ending, and that it arrives without waiting on anybody, the hop is
    /// cancelled while the main actor still holds this function, so it is cancelled before its
    /// first instruction runs, and returning at once is the evidence no browser was presented.
    @Test func anEscapedHopEndsAsACancellation() async {
        let signIn = AllodiaSignIn()
        let hop = Task { @MainActor in
            try await signIn.authorize(authorizationURL: "https://example.invalid/authorize")
        }
        hop.cancel()

        await #expect(throws: AllodiaSignInError.cancelled) { try await hop.value }
    }
}
