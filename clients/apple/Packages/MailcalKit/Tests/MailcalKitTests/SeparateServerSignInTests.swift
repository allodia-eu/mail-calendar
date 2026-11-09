// A separate calendar or address book server signed in at its own provider (docs/mail-oauth.md
// rule 13): the sign-in is offered only once the core draws it; an answer never takes away a
// password route somebody has begun; an attempt
// superseded or retired changes nothing; and a success leads where Connect's success leads.
// That the button and the way back are on screen when the state says so stays hand-verified,
// because reaching them opens a real browser.

import MailcalBindings
import Testing

@testable import MailcalUI

private let ask = separateAsk([.calendar, .contacts])
private let offering = separateAsk([.calendar, .contacts], signIn: .provider)

@Suite struct SeparateServerSignInOfferTests {
    @Test func theFormIsThePasswordFormUntilTheServerAnswers() {
        let form = SeparateServerSignIn()
        #expect(!form.offersSignIn(ask))
        #expect(!form.signInLeads(ask))
        #expect(form.showsPasswordFields(ask))
    }

    @Test func aSignInTheCoreDrawsLeadsAndPutsThePasswordBehindIt() {
        var form = SeparateServerSignIn()
        form.offered(started: false)
        #expect(form.offersSignIn(offering))
        #expect(form.signInLeads(offering))
        #expect(!form.showsPasswordFields(offering))
        #expect(!form.offersNextcloud(offering))
        #expect(form.offersNextcloud(separateAsk([.calendar], signIn: .nextcloud)))

        // "Use a password instead" draws the fields beneath a sign-in that stays.
        form.choosePassword()
        #expect(form.offersSignIn(offering))
        #expect(!form.signInLeads(offering))
        #expect(form.showsPasswordFields(offering))
    }

    @Test func anAnswerKeepsThePasswordRouteSomebodyHasBegun() {
        #expect(!SeparateServerRules.passwordStarted(ask, username: "alice", password: ""))
        #expect(!SeparateServerRules.passwordStarted(ask, username: "alice ", password: ""))
        #expect(SeparateServerRules.passwordStarted(ask, username: "alice", password: "s"))
        #expect(SeparateServerRules.passwordStarted(ask, username: "alice2", password: ""))

        var form = SeparateServerSignIn()
        form.offered(started: true)
        #expect(form.offersSignIn(offering))
        #expect(!form.signInLeads(offering))
        #expect(form.showsPasswordFields(offering))
    }
}

@Suite struct SeparateServerSignInAttemptTests {
    private func offered() -> SeparateServerSignIn {
        var form = SeparateServerSignIn()
        form.offered(started: false)
        return form
    }

    @Test func aSupersededAttemptsAnswerDoesNothing() {
        var form = offered()
        let abandoned = form.begin()
        let fresh = form.begin()
        let abandonedMoved = form.finish(abandoned, .moved)
        #expect(abandonedMoved == nil)
        let abandonedFailed = form.finish(abandoned, .failed("late"))
        #expect(abandonedFailed == nil)
        #expect(form.signingIn)
        #expect(form.signInLeads(offering))
        let freshMoved = form.finish(fresh, .moved)
        #expect(freshMoved == .done)
        #expect(!form.signingIn)
    }

    @Test func aRetiredAttemptsAnswerDoesNothing() {
        var form = offered()
        let attempt = form.begin()
        form.attempts.outlasted(attempt)
        #expect(form.escapable)
        form.retire()
        #expect(!form.signingIn)
        #expect(!form.escapable)

        // A redirect or a completion arriving afterwards belongs to no screen.
        form.redirected(attempt)
        #expect(!form.completing)
        let lateMoved = form.finish(attempt, .moved)
        #expect(lateMoved == nil)
        let lateFailed = form.finish(attempt, .failed("late"))
        #expect(lateFailed == nil)
        #expect(form.signInLeads(offering))
    }

    @Test func theExchangeAndAddDrawNoWayBack() {
        var form = offered()
        let attempt = form.begin()
        form.attempts.outlasted(attempt)
        form.redirected(attempt)
        #expect(form.completing)
        #expect(!form.escapable)
        let moved = form.finish(attempt, .moved)
        #expect(moved == .done)
        #expect(!form.completing)
    }

    @Test func aSupersededAttemptsRedirectDoesNotHoldTheFresh() {
        var form = offered()
        let abandoned = form.begin()
        _ = form.begin()
        form.redirected(abandoned)
        #expect(!form.completing)
    }

    @Test func aDismissedBrowserEndsTheBusyStateAndSaysNothing() {
        var form = offered()
        let attempt = form.begin()
        let dismissed = form.finish(attempt, .cancelled)
        #expect(dismissed == .stay(failure: nil))
        #expect(!form.signingIn)
        #expect(form.signInLeads(offering))
    }

    @Test func aFailureIsSaidAndBringsThePasswordRouteBack() {
        var form = offered()
        let attempt = form.begin()
        let failed = form.finish(attempt, .failed("no"))
        #expect(failed == .stay(failure: "no"))
        #expect(form.showsPasswordFields(offering))
        #expect(form.offersSignIn(offering))
    }
}

@Suite struct SeparateServerSignInOutcomeTests {
    @Test func aSuccessEndsTheFormAndAFailureKeepsIt() {
        #expect(SeparateServerRules.afterSignIn(.moved) == .done)
        #expect(SeparateServerRules.afterSignIn(.cancelled) == .stay(failure: nil))
        #expect(SeparateServerRules.afterSignIn(.failed("refused")) == .stay(failure: "refused"))
    }

    @Test func aFailureReadsThroughTheExistingMapping() {
        #expect(
            SeparateServerRules.signInFailure(MailcalError.Connect("no refresh token"))
                == L10n.setup_imap_signin_failed())
        #expect(
            SeparateServerRules.signInFailure(MailcalError.DifferentAccount)
                == L10n.signin_different_account())
        #expect(
            SeparateServerRules.signInFailure(MailcalError.Config("bad"))
                == coreMessage(MailcalError.Config("bad")))
    }

    @Test func aSignInFromAnotherRequestSaysSoRatherThanThatSigningInFailed() {
        #expect(SeparateServerRules.signInFailure(MailcalError.StaleSignIn) == L10n.signin_stale())
    }

    @Test func aUseThatDidNotConnectSaysWhichAndWhy() {
        #expect(
            SeparateServerRules.signInFailure(
                MailcalError.UseNotConnected(capability: .contacts, problem: .unreachable))
                == L10n.separate_server_unreachable_contacts())
        #expect(
            SeparateServerRules.signInFailure(
                MailcalError.UseNotConnected(capability: .calendar, problem: .needsOwnSignIn))
                == L10n.settings_account_use_own_sign_in_calendar())
    }
}

@Suite struct SeparateServerNextcloudOfferTests {
    private let nextcloud = separateAsk([.calendar, .contacts], signIn: .nextcloud)

    @Test func aNextcloudServerLeadsWithItsSignInAndPutsThePasswordBehindIt() {
        var form = SeparateServerSignIn()
        form.offered(started: false)
        #expect(form.offersNextcloud(nextcloud))
        #expect(!form.offersSignIn(nextcloud))
        #expect(form.signInLeads(nextcloud))
        #expect(!form.showsPasswordFields(nextcloud))

        form.choosePassword()
        #expect(form.offersNextcloud(nextcloud))
        #expect(form.showsPasswordFields(nextcloud))
    }

    @Test func aPasswordRouteBegunKeepsItsFields() {
        var form = SeparateServerSignIn()
        form.offered(started: true)
        #expect(form.offersNextcloud(nextcloud))
        #expect(form.showsPasswordFields(nextcloud))
    }
}
