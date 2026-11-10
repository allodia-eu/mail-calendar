// What this client still decides about an account's browser sign-in, which the core holds
// (docs/waiting.md, docs/provider-oauth.md rule 13): which redirect it asks the core for, how it
// waits for the browser's return, when it opens and closes a browser, and how the Sign-in section
// draws the core's record. Every step of the sign-in is the core's, tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

private func browser(
    _ stage: BrowserSignInStage, attempt: UInt64 = 7, cancel: Bool = true
) -> BrowserSignIn {
    BrowserSignIn(
        attempt: attempt, account: "alice@example.org@imap.example.org", stage: stage,
        cancel: ControlState(enabled: cancel, running: false))
}

private func section(_ browser: BrowserSignIn?, control: ControlState) -> AccountSignIn {
    AccountSignIn(
        line: .provider, routes: [.signInAgain], control: control, password: nil,
        switches: false, browser: browser)
}

private let inBrowser = BrowserSignInStage.inBrowser(
    url: "https://login.example/authorize", redirectUri: "eu.example.mail://imap-oauth")

@Suite struct BrowserSignInRedirectTests {
    @Test func eachKindAsksForTheRedirectSetupUsesForIt() {
        #expect(
            AccountSignInRules.redirectURI(.microsoft, google: nil)
                == MicrosoftOAuthConfig.redirectURI)
        #expect(AccountSignInRules.redirectURI(.imap, google: nil) == ImapOAuthConfig.redirectURI)
        #expect(AccountSignInRules.redirectURI(.jmap, google: nil) == JmapOAuthConfig.redirectURI)
        #expect(AccountSignInRules.redirectURI(.dav, google: nil) == "")
    }

    @Test func aGoogleAccountReturnsToTheRegisteredRedirectOrToALoopbackBoundForIt() {
        #expect(
            AccountSignInRules.redirectURI(.google, google: "com.example.apps.1:/oauth")
                == "com.example.apps.1:/oauth")
        #expect(AccountSignInRules.redirectURI(.google, google: nil) == nil)
    }
}

@Suite struct BrowserSignInListenerTests {
    @Test func aRedirectToASchemeIsCaughtByTheAuthSession() {
        #expect(
            AccountSignInRules.listener(redirect: MicrosoftOAuthConfig.redirectURI, loopback: nil)
                == .session(callbackScheme: MicrosoftOAuthConfig.callbackScheme))
        #expect(
            AccountSignInRules.listener(redirect: JmapOAuthConfig.redirectURI, loopback: nil)
                == .session(callbackScheme: JmapOAuthConfig.callbackScheme))
        #expect(
            AccountSignInRules.listener(redirect: "com.example.apps.1:/oauth", loopback: nil)
                == .session(callbackScheme: "com.example.apps.1"))
    }

    @Test func theLoopbackBoundForTheSignInWaitsForItsOwnAddressOnly() {
        let bound = "http://127.0.0.1:50123/"
        #expect(AccountSignInRules.listener(redirect: bound, loopback: bound) == .loopback)
        #expect(
            AccountSignInRules.listener(redirect: "http://127.0.0.1:50999/", loopback: bound)
                == nil)
        #expect(AccountSignInRules.listener(redirect: bound, loopback: nil) == nil)
    }

    @Test func aSignInThatReturnsNothingWaitsOnTheServer() {
        #expect(AccountSignInRules.listener(redirect: nil, loopback: nil) == .server)
    }

    @Test func noAddressCannotBeWaitedFor() {
        #expect(AccountSignInRules.listener(redirect: "", loopback: nil) == nil)
        #expect(
            AccountSignInRules.listener(redirect: "https://example.org/back", loopback: nil) == nil)
    }
}

@Suite struct BrowserSignInDriverTests {
    @Test func theBrowserOpensOnceTheCoreWaitsOnItAndOnlyOncePerAttempt() {
        #expect(AccountSignInRules.opening(nil, opened: false) == nil)
        #expect(AccountSignInRules.opening(browser(.starting), opened: false) == nil)
        #expect(
            AccountSignInRules.opening(browser(.asking(unreached: [.calendar])), opened: false)
                == nil)
        #expect(
            AccountSignInRules.opening(browser(inBrowser), opened: false)
                == SignInOpening(
                    url: "https://login.example/authorize",
                    redirect: "eu.example.mail://imap-oauth"))
        #expect(AccountSignInRules.opening(browser(inBrowser), opened: true) == nil)
    }

    @Test func aBrowserTheCoreNoLongerWaitsOnIsClosed() {
        // Still waited on.
        #expect(AccountSignInRules.keeps(browser(inBrowser), attempt: 7, opened: true))
        // Superseded, cancelled or over.
        #expect(
            !AccountSignInRules.keeps(browser(inBrowser, attempt: 8), attempt: 7, opened: true))
        #expect(!AccountSignInRules.keeps(nil, attempt: 7, opened: true))
        // The core completes it, or a Nextcloud server granted access: the sheet closes.
        #expect(!AccountSignInRules.keeps(browser(.completing), attempt: 7, opened: true))
    }

    @Test func aLoopbackBoundBeforeThePressWaitsForItsBrowser() {
        #expect(AccountSignInRules.keeps(browser(.starting), attempt: 7, opened: false))
        #expect(!AccountSignInRules.keeps(nil, attempt: 7, opened: false))
    }

    @Test func eachAnswerToTheQuestionIsHandedToTheCore() {
        #expect(AccountSignInRules.answer(.keep) == .keepOnPassword)
        #expect(AccountSignInRules.answer(.stop) == .stopUsing)
        #expect(AccountSignInRules.answer(.cancel) == nil)
    }
}

@Suite struct BrowserSignInSectionTests {
    @Test func theSectionWaitsOnTheBrowserWithACancel() {
        let drawn = section(
            browser(inBrowser), control: ControlState(enabled: true, running: false))
        #expect(
            AccountSettingsLabels.signInWaiting(drawn.browser, name: "alice@example.org")
                == L10n.settings_account_signin_waiting(email: "alice@example.org"))
        #expect(drawn.browser?.cancel.enabled == true)
        // A route pressed now starts the sign-in again.
        #expect(drawn.control.enabled)
    }

    @Test func aNextcloudSignInWaitsOnTheServerInItsOwnWords() {
        let drawn = section(
            browser(.inBrowser(url: "https://cloud.example/login", redirectUri: nil)),
            control: ControlState(enabled: true, running: false))
        #expect(
            AccountSettingsLabels.signInWaiting(drawn.browser, name: "alice@example.org")
                == L10n.nextcloud_sign_in_waiting())
    }

    @Test func aSignInStartingOrCompletingRunsItsRouteAndSaysNoWaitingLine() {
        let running = ControlState(enabled: false, running: true)
        for stage in [BrowserSignInStage.starting, .completing] {
            let drawn = section(browser(stage, cancel: stage == .starting), control: running)
            #expect(AccountSettingsLabels.signInWaiting(drawn.browser, name: "alice") == nil)
            #expect(drawn.control.running)
        }
        #expect(AccountSettingsLabels.signInWaiting(nil, name: "alice") == nil)
    }

    @Test func theQuestionIsAskedForItsAttemptWhileTheCoreAsksIt() {
        #expect(
            AccountSignInRules.question(browser(.asking(unreached: [.calendar, .contacts])))
                == SignInQuestion(attempt: 7, set: .both))
        #expect(AccountSignInRules.question(browser(inBrowser)) == nil)
        #expect(AccountSignInRules.question(nil) == nil)
    }
}
