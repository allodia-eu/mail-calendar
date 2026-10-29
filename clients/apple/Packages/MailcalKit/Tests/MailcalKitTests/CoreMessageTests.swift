// What a person reads for a core error. A refused sign-in is said in their language rather than as
// the server's reply, which is a status and a body meant for a developer, and so is a sign-in the
// browser returned from another request for, and a use that did not connect to a server of its own.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct CoreMessageTests {
    @Test func aRefusedSignInIsSaidInWords() {
        let refused = MailcalError.SignInRefused(reason: "JMAP HTTP 401: {\"status\":401}")
        #expect(coreMessage(refused) == L10n.signin_refused())
    }

    @Test func anAccountAlreadySetUpIsSaidInWords() {
        let twice = MailcalError.AlreadySetUp(accountId: "alice@example.org@imap.example.org")
        #expect(coreMessage(twice) == L10n.account_already_set_up())
    }

    @Test func aSignInAsAnotherAccountIsSaidInWords() {
        #expect(coreMessage(MailcalError.DifferentAccount) == L10n.signin_different_account())
    }

    @Test func aSignInFromAnotherRequestIsSaidInWords() {
        #expect(coreMessage(MailcalError.StaleSignIn) == L10n.signin_stale())
    }

    @Test func aCalendarThatDidNotConnectSaysWhy() {
        #expect(
            notConnected(.calendar, .unreachable) == L10n.separate_server_unreachable_calendar())
        #expect(
            notConnected(.calendar, .needsOwnSignIn)
                == L10n.settings_account_use_own_sign_in_calendar())
        #expect(notConnected(.calendar, .signInRefused) == L10n.signin_refused())
        #expect(
            notConnected(.calendar, .serverChanged)
                == L10n.settings_account_use_server_changed_calendar())
    }

    @Test func anAddressBookThatDidNotConnectSaysWhy() {
        #expect(
            notConnected(.contacts, .unreachable) == L10n.separate_server_unreachable_contacts())
        #expect(
            notConnected(.contacts, .needsOwnSignIn)
                == L10n.settings_account_use_own_sign_in_contacts())
        #expect(notConnected(.contacts, .signInRefused) == L10n.signin_refused())
        #expect(
            notConnected(.contacts, .serverChanged)
                == L10n.settings_account_use_server_changed_contacts())
    }

    @Test func mailThatDidNotConnectReadsAsTheCalendar() {
        #expect(notConnected(.mail, .unreachable) == L10n.separate_server_unreachable_calendar())
    }

    private func notConnected(_ capability: AccountCapability, _ problem: UseProblem) -> String {
        coreMessage(MailcalError.UseNotConnected(capability: capability, problem: problem))
    }
}

/// A failure signing in again settles is said in its own words wherever a sign-in is offered,
/// including a banner, which says anything else as one plain line.
@Suite struct SignInRemedyTests {
    @Test func onlyAnotherRequestOrAnotherAccountIsARemedy() {
        #expect(signInRemedy(MailcalError.StaleSignIn) == L10n.signin_stale())
        #expect(signInRemedy(MailcalError.DifferentAccount) == L10n.signin_different_account())
        #expect(signInRemedy(MailcalError.Connect("token endpoint said 400")) == nil)
        #expect(signInRemedy(MailcalError.SignInRefused(reason: "401")) == nil)
        #expect(
            signInRemedy(MailcalError.UseNotConnected(capability: .calendar, problem: .unreachable))
                == nil)
    }

    @Test func aBannerSaysARemedyInItsOwnWords() {
        #expect(ConsentEnded.failure(MailcalError.StaleSignIn).bannerNotice == L10n.signin_stale())
        #expect(
            ConsentEnded.failure(MailcalError.DifferentAccount).bannerNotice
                == L10n.signin_different_account())
    }

    @Test func aBannerSaysAnyOtherFailureAsOnePlainLine() {
        #expect(
            ConsentEnded.failure(MailcalError.Connect("invalid_grant")).bannerNotice
                == L10n.signin_expired_failed())
        #expect(ConsentEnded.failed("no browser").bannerNotice == L10n.signin_expired_failed())
    }

    @Test func aBannerSaysNothingUnlessTheSignInFailed() {
        #expect(ConsentEnded.signedIn.bannerNotice == nil)
        #expect(ConsentEnded.cancelled.bannerNotice == nil)
    }

    @Test func thePageSaysTheSameDetailEitherWay() {
        #expect(
            ConsentEnded.failure(MailcalError.StaleSignIn)
                == .failed(L10n.signin_stale(), remedy: true))
        #expect(
            ConsentEnded.failure(MailcalError.Config("bad"))
                == .failed(coreMessage(MailcalError.Config("bad")), remedy: false))
    }
}
