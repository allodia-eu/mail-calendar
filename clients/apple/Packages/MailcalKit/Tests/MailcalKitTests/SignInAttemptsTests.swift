// The way out of a setup sign-in that does not come back (docs/onboarding.md, "Nobody may be
// stranded on it"), and a fresh press superseding one still out (docs/provider-oauth.md rule 13).
// What is pinned here is the logic; that the Cancel is on screen when the state says so stays
// hand-verified, because reaching it opens a real browser.

import Testing

@testable import MailcalUI

struct SignInAttemptsTests {
    @Test func theWayBackIsDrawnOnlyOnceTheAttemptInFlightOutlastsTheThreshold() {
        var attempts = SignInAttempts()
        let first = attempts.begin()
        #expect(attempts.signingIn)
        #expect(!attempts.escapable)

        attempts.outlasted(first)
        #expect(attempts.escapable)

        // A newer attempt starts its own clock; the older one's never arms it.
        let second = attempts.begin()
        #expect(!attempts.escapable)
        attempts.outlasted(first)
        #expect(!attempts.escapable)
        attempts.outlasted(second)
        #expect(attempts.escapable)
    }

    @Test func anEscapedAttemptIsRetiredAndItsLateAnswerDropped() {
        var attempts = SignInAttempts()
        let attempt = attempts.begin()
        attempts.outlasted(attempt)
        attempts.escape()
        #expect(!attempts.signingIn)
        #expect(!attempts.escapable)

        // The browser or the core answering afterwards changes nothing on screen.
        let ended = attempts.finish(attempt)
        #expect(!ended)
        attempts.outlasted(attempt)
        #expect(!attempts.escapable)
    }

    @Test func aFreshPressSupersedesTheAttemptStillOut() {
        var attempts = SignInAttempts()
        let abandoned = attempts.begin()
        let fresh = attempts.begin()
        #expect(abandoned != fresh)

        // The abandoned attempt ending must not end the fresh one.
        let abandonedEnded = attempts.finish(abandoned)
        #expect(!abandonedEnded)
        #expect(attempts.signingIn)
        let freshEnded = attempts.finish(fresh)
        #expect(freshEnded)
        #expect(!attempts.signingIn)
    }

    @Test func anAttemptThatAnswersEndsTheBusyState() {
        var attempts = SignInAttempts()
        let attempt = attempts.begin()
        attempts.outlasted(attempt)
        let ended = attempts.finish(attempt)
        #expect(ended)
        #expect(!attempts.signingIn)
        #expect(!attempts.escapable)
    }
}

@MainActor
struct JmapSignInEscapeTests {
    /// Escaping ends the hop as a cancellation, and without waiting on anybody: the hop is
    /// cancelled while the main actor still holds this function, so returning at once is the
    /// evidence no browser was presented.
    @Test func anEscapedHopEndsAsACancellation() async {
        let signIn = JmapSignIn()
        let hop = Task { @MainActor in
            try await signIn.authorize(authorizationURL: "https://example.invalid/authorize")
        }
        hop.cancel()

        await #expect(throws: JmapSignInError.cancelled) { try await hop.value }
    }
}
