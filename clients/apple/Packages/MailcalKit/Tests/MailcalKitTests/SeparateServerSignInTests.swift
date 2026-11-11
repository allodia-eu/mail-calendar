// A separate calendar or address book server signed in through the browser, at its own provider
// (docs/mail-oauth.md rule 13) or with Nextcloud: the sign-in is offered only once the core draws
// it, and an answer never takes away a password route somebody has begun. The sign-in itself is
// the core's (tested there); what this client decides is how the form draws its section: the
// button running as the core says, the line and its Cancel while the browser waits, the fields
// held while a sign-in is out, and leaving the form cancelling one still out. That the controls
// are on screen when the state says so stays hand-verified, because reaching them opens a real
// browser.

import MailcalBindings
import Testing

@testable import MailcalUI

private let ask = separateAsk([.calendar, .contacts])
private let offering = separateAsk([.calendar, .contacts], signIn: .provider)
private let nextcloud = separateAsk([.calendar, .contacts], signIn: .nextcloud)

private let held = ControlState(enabled: false, running: false)
private let running = ControlState(enabled: false, running: true)

private let atProvider = BrowserSignInStage.inBrowser(
    url: "https://dav.example.org/authorize", redirectUri: ImapOAuthConfig.redirectURI)
private let atNextcloud = BrowserSignInStage.inBrowser(
    url: "https://dav.example.org/login/v2/flow", redirectUri: nil)

private func browser(_ stage: BrowserSignInStage, cancel: Bool = true) -> BrowserSignIn {
    BrowserSignIn(
        attempt: 7, account: "alice@imap", stage: stage,
        cancel: ControlState(enabled: cancel, running: false))
}

/// The form `from` draws while the core holds a sign-in at `stage`: Connect and Skip held, the
/// sign-in's control as `control`.
private func out(
    _ from: SeparateServerAsk, _ stage: BrowserSignInStage, control: ControlState = .free()
) -> SeparateServerAsk {
    var section = from.section
    section.connect = held
    section.stopUsing = held
    section.signInControl = control
    section.browser = browser(stage, cancel: stage != .completing)
    return SeparateServerAsk(account: from.account, section: section)
}

@Suite struct SeparateServerSignInOfferTests {
    @Test func theFormIsThePasswordFormUntilTheServerAnswers() {
        let form = SeparateServerSignIn()
        #expect(form.offers(ask) == nil)
        #expect(!form.signInLeads(ask))
        #expect(form.showsPasswordFields(ask))
    }

    @Test func aWayInTheCoreDrawsLeadsAndPutsThePasswordBehindIt() {
        for drawn in [offering, nextcloud] {
            var form = SeparateServerSignIn()
            form.drawn(ask.section, drawn.section, started: false)
            #expect(form.offers(drawn) == drawn.section.signIn)
            #expect(form.signInLeads(drawn))
            #expect(!form.showsPasswordFields(drawn))

            // "Use a password instead" draws the fields beneath a sign-in that stays.
            form.choosePassword()
            #expect(form.offers(drawn) == drawn.section.signIn)
            #expect(!form.signInLeads(drawn))
            #expect(form.showsPasswordFields(drawn))
        }
    }

    @Test func anAnswerKeepsThePasswordRouteSomebodyHasBegun() {
        #expect(!SeparateServerRules.passwordStarted(ask, username: "alice", password: ""))
        #expect(!SeparateServerRules.passwordStarted(ask, username: "alice ", password: ""))
        #expect(SeparateServerRules.passwordStarted(ask, username: "alice", password: "s"))
        #expect(SeparateServerRules.passwordStarted(ask, username: "alice2", password: ""))

        for drawn in [offering, nextcloud] {
            var form = SeparateServerSignIn()
            form.drawn(ask.section, drawn.section, started: true)
            #expect(form.offers(drawn) != nil)
            #expect(!form.signInLeads(drawn))
            #expect(form.showsPasswordFields(drawn))
        }
    }

    @Test func eachWayInIsNamedInItsOwnWords() {
        #expect(SeparateServerLabels.signInButton(.provider) == L10n.setup_imap_signin_button())
        #expect(SeparateServerLabels.signInButton(.nextcloud) == L10n.nextcloud_sign_in_button())
        #expect(
            SeparateServerLabels.signInLine(.provider, offering.section)
                == L10n.setup_imap_signin_note())
        #expect(
            SeparateServerLabels.signInLine(.nextcloud, nextcloud.section)
                == L10n.nextcloud_sign_in_note())
    }
}

@Suite @MainActor struct SeparateServerBrowserSignInTests {
    @Test func aSignInStartingRunsItsButtonAndHoldsTheFields() {
        let form = SeparateServerFormModel(out(offering, .starting, control: running))
        #expect(form.section.signInControl.running)
        #expect(!form.section.connect.enabled)
        #expect(!form.section.stopUsing.enabled)
        // No line about the browser before it is up, and no Cancel beside it.
        #expect(SeparateServerRules.waiting(form.section) == nil)
        #expect(
            SeparateServerLabels.signInLine(.provider, form.section)
                == L10n.setup_imap_signin_note())
    }

    @Test func whileTheBrowserWaitsTheLineSaysSoWithItsCancel() {
        let form = SeparateServerFormModel(out(offering, atProvider))
        let waiting = SeparateServerRules.waiting(form.section)
        #expect(waiting?.attempt == 7)
        #expect(waiting?.cancel.enabled == true)
        #expect(
            SeparateServerLabels.signInLine(.provider, form.section)
                == L10n.settings_account_signin_waiting(email: "alice"))
        // A press now starts a fresh sign-in, as after a tab closed by accident.
        #expect(form.section.signInControl.enabled)
        #expect(!form.section.signInControl.running)
        #expect(!form.section.connect.enabled)
    }

    @Test func aNextcloudSignInWaitsInItsOwnWords() {
        let form = SeparateServerFormModel(out(nextcloud, atNextcloud))
        #expect(SeparateServerRules.waiting(form.section)?.attempt == 7)
        #expect(
            SeparateServerLabels.signInLine(.nextcloud, form.section)
                == L10n.nextcloud_sign_in_waiting())
    }

    @Test func leavingTheFormCancelsASignInStillOut() {
        for stage in [BrowserSignInStage.starting, atProvider] {
            let form = SeparateServerFormModel(out(offering, stage))
            #expect(form.cancelsOnLeaving?.attempt == 7)
            #expect(form.canLeave)
        }
        // The core no longer lets go of one it completes, so the form waits for it.
        let completing = SeparateServerFormModel(out(offering, .completing, control: running))
        #expect(completing.cancelsOnLeaving == nil)
        #expect(!completing.canLeave)
        #expect(SeparateServerRules.waiting(completing.section) == nil)

        let idle = SeparateServerFormModel(offering)
        #expect(idle.cancelsOnLeaving == nil)
        #expect(idle.canLeave)
    }

    @Test func aFailureIsSaidAsTheRefusalAndBringsThePasswordRouteBack() {
        let form = SeparateServerFormModel(offering)
        #expect(form.said == nil)
        #expect(form.signIn.signInLeads(offering))

        var refused = offering.section
        refused.failed = .browserNotOpened
        let failed = SeparateServerAsk(account: offering.account, section: refused)
        form.drawn(failed)
        #expect(form.said == L10n.account_signin_browser_failed())
        #expect(form.signIn.showsPasswordFields(failed))
        #expect(form.signIn.offers(failed) == .provider)
    }
}
