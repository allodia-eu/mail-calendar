// The Allodia account's card as this client draws it from the core's record (`allodiaCard`):
// which face Settings shows, when the first-run card offers its way back, and the words for what a
// sign-in or sign-out came to. Every step of the sign-in is the core's, tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

private let anna = AllodiaAccount(id: "acct-1", email: "anna@example.org", name: "Anna")

private let out = BrowserSignIn(
    attempt: 3, account: "allodia-account",
    stage: .inBrowser(
        url: "https://id.example/authorize", redirectUri: AllodiaOAuthConfig.redirectURI),
    cancel: ControlState(enabled: true, running: false))

private func card(
    _ state: AllodiaCardState = .signedOut,
    browser: BrowserSignIn? = nil,
    failure: AllodiaFailure? = nil,
    onboarding: AllodiaOnboarding = .offering
) -> AllodiaCard {
    AllodiaCard(
        state: state, browser: browser, signIn: .free(), create: .free(), signOut: .free(),
        failure: failure, endSession: nil, onboarding: onboarding)
}

@Suite struct AllodiaCardTests {
    @Test func signedOutOffersBothRoutes() {
        #expect(AllodiaCardRules.face(card()) == .signedOut)
    }

    @Test func aSignInOutIsDrawnAsSigningInWithItsCancelWhateverTheState() {
        #expect(AllodiaCardRules.face(card(browser: out)) == .signingIn(out))
        #expect(AllodiaCardRules.face(card(.ended(account: anna), browser: out)) == .signingIn(out))
    }

    @Test func signedInAndEndedNameTheAccount() {
        #expect(AllodiaCardRules.face(card(.signedIn(account: anna))) == .signedIn(anna))
        #expect(AllodiaCardRules.face(card(.ended(account: anna))) == .ended(anna))
    }

    /// The first-run card is the one screen nobody can skip, so its way back is drawn, and drawn
    /// only, once the core says the hop has run long enough (docs/onboarding.md).
    @Test func theFirstRunCardOffersItsWayBackOnlyWhenTheCoreSaysSo() {
        let early = card(browser: out, onboarding: .signingIn(escape: false))
        let late = card(browser: out, onboarding: .signingIn(escape: true))
        #expect(AllodiaCardRules.escape(early) == nil)
        #expect(AllodiaCardRules.escape(late) == out)
        #expect(AllodiaCardRules.escape(card(onboarding: .checking)) == nil)
    }

    @Test func aFailureIsWordedTheSameOnBothCards() {
        #expect(AllodiaCardRules.failure(.signIn(refusal: .staleSignIn)) == L10n.signin_stale())
        #expect(
            AllodiaCardRules.failure(.signIn(refusal: .browserNotOpened))
                == L10n.settings_allodia_browser_failed())
        #expect(
            AllodiaCardRules.failure(.signIn(refusal: .notFinished))
                == L10n.settings_allodia_timeout())
        #expect(
            AllodiaCardRules.failure(.signIn(refusal: .signInRefused))
                == L10n.settings_allodia_failed(error: L10n.signin_refused()))
        #expect(
            AllodiaCardRules.failure(.signOut(detail: "keychain said no"))
                == L10n.settings_allodia_failed(error: "keychain said no"))
    }

    /// The core's sign-in returns where this client already listened for the Allodia sign-in, and
    /// the browser driver waits for that scheme.
    @Test func theDriverWaitsForTheAllodiaRedirect() {
        #expect(
            AccountSignInRules.listener(redirect: AllodiaOAuthConfig.redirectURI, loopback: nil)
                == .session(callbackScheme: AllodiaOAuthConfig.callbackScheme))
        #expect(AllodiaOAuthConfig.callbackScheme == Brand.appID)
    }
}
