// A separate calendar or address book server signed in at its own provider (docs/mail-oauth.md
// rule 13): the sign-in is offered only once the server said so, and only for the form it was
// asked about; an answer never takes away a password route somebody has begun; an attempt
// superseded or retired changes nothing; and a success leads where Connect's success leads.
// That the button and the way back are on screen when the state says so stays hand-verified,
// because reaching them opens a real browser.

import MailcalBindings
import Testing

@testable import MailcalUI

private let ask = SeparateServerAsk(
    account: "alice@imap", uses: [.calendar, .contacts], host: "dav.example.org",
    username: "alice")

@Suite struct SeparateServerSignInOfferTests {
    @Test func theFormIsThePasswordFormUntilTheServerAnswers() {
        let form = SeparateServerSignIn()
        #expect(!form.offersSignIn(ask))
        #expect(!form.signInLeads(ask))
        #expect(form.showsPasswordFields(ask))
    }

    @Test func aNoChangesNothing() {
        var form = SeparateServerSignIn()
        form.answered(SeparateServerOffer(ask, offers: false), started: false)
        #expect(!form.offersSignIn(ask))
        #expect(form.showsPasswordFields(ask))
    }

    @Test func aYesLeadsWithTheSignInAndPutsThePasswordBehindIt() {
        var form = SeparateServerSignIn()
        form.answered(SeparateServerOffer(ask, offers: true), started: false)
        #expect(form.offersSignIn(ask))
        #expect(form.signInLeads(ask))
        #expect(!form.showsPasswordFields(ask))

        // "Use a password instead" draws the fields beneath a sign-in that stays.
        form.choosePassword()
        #expect(form.offersSignIn(ask))
        #expect(!form.signInLeads(ask))
        #expect(form.showsPasswordFields(ask))
    }

    @Test func anAnswerForAnotherFormIsNotThisOnes() {
        var form = SeparateServerSignIn()
        let calendarOnly = SeparateServerAsk(
            account: ask.account, uses: [.calendar], host: ask.host, username: ask.username)
        form.answered(SeparateServerOffer(calendarOnly, offers: true), started: false)
        #expect(form.offersSignIn(calendarOnly))
        #expect(!form.offersSignIn(ask))
        #expect(form.showsPasswordFields(ask))

        let otherMailbox = SeparateServerAsk(
            account: "bob@imap", uses: ask.uses, host: ask.host, username: ask.username)
        #expect(!SeparateServerOffer(otherMailbox, offers: true).answers(ask))
    }

    @Test func anAnswerKeepsThePasswordRouteSomebodyHasBegun() {
        #expect(!SeparateServerRules.passwordStarted(ask, username: "alice", password: ""))
        #expect(!SeparateServerRules.passwordStarted(ask, username: "alice ", password: ""))
        #expect(SeparateServerRules.passwordStarted(ask, username: "alice", password: "s"))
        #expect(SeparateServerRules.passwordStarted(ask, username: "alice2", password: ""))

        var form = SeparateServerSignIn()
        form.answered(SeparateServerOffer(ask, offers: true), started: true)
        #expect(form.offersSignIn(ask))
        #expect(!form.signInLeads(ask))
        #expect(form.showsPasswordFields(ask))
    }
}

@Suite struct SeparateServerSignInAttemptTests {
    private func offered() -> SeparateServerSignIn {
        var form = SeparateServerSignIn()
        form.answered(SeparateServerOffer(ask, offers: true), started: false)
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
        #expect(form.signInLeads(ask))
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
        #expect(form.signInLeads(ask))
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
        #expect(form.signInLeads(ask))
    }

    @Test func aFailureIsSaidAndBringsThePasswordRouteBack() {
        var form = offered()
        let attempt = form.begin()
        let failed = form.finish(attempt, .failed("no"))
        #expect(failed == .stay(failure: "no"))
        #expect(form.showsPasswordFields(ask))
        #expect(form.offersSignIn(ask))
    }
}

@Suite struct SeparateServerSignInOutcomeTests {
    @Test func aSuccessLeadsWhereConnectsSuccessLeads() {
        #expect(SeparateServerRules.afterSignIn(.moved) == SeparateServerRules.afterConnect(nil))
        #expect(SeparateServerRules.afterConnect(nil) == .done)
        #expect(SeparateServerRules.afterConnect("refused") == .stay(failure: "refused"))
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
}
