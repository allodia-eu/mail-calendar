// The Allodia account on the model: its card as the core keeps it, and the changes asked of it.
// The core runs the sign-in from the press to the grant it stores, as one browser sign-in the
// driver opens like an account's (MailcalModel.BrowserSignIn.swift), and the sign-out on a thread
// of its own; this host never sees or stores a token, and keeps no step of either.

import Foundation
import MailcalBindings

/// The Allodia account's card as the core last drew it, and what this host did about it.
struct AllodiaCardHeld: Equatable {
    /// `nil` until the core is open.
    var card: AllodiaCard?
    /// The last end-of-session page opened, so each opens once.
    var endSessionOpened: UInt64 = 0
    /// The subscription section as the core last drew it, read on the same signal.
    var subscription: AllodiaSubscriptionCard?
}

extension MailboxModel {
    /// Signs in in the browser, or with `create` at the service's sign-up page: from Settings,
    /// the first-run card, the Allodia notice and the sections that ask for a sign-in again. It
    /// supersedes a sign-in still out.
    func signInToAllodia(create: Bool = false) {
        app?.dispatch(intent: .allodia(
            intent: .signIn(create: create, redirectUri: AllodiaOAuthConfig.redirectURI)))
        driveBrowserSignIn()
    }

    /// Stops the sign-in `browser` is, from its Cancel; it then says nothing.
    func cancelAllodiaSignIn(_ browser: BrowserSignIn) {
        app?.dispatch(intent: .allodia(intent: .cancelSignIn(attempt: browser.attempt)))
    }

    /// Signs out. Local, as removing a mail account is: the grant stays alive at the service
    /// until it expires or the person revokes it there.
    func signOutOfAllodia() {
        app?.dispatch(intent: .allodia(intent: .signOut))
    }

    /// What Sign in, wherever a section offers it, is drawn from.
    var allodiaSignInControl: ControlState {
        allodia.card?.signIn ?? .free(false)
    }

    /// Reads the card and the subscription section again, on an Allodia signal, an OtherDevices
    /// one and at connect. Opens the service's end-of-session page once for each sign-out that
    /// names one.
    func refreshAllodiaCard() {
        guard let app else { return }
        let fresh = app.allodiaCard()
        if let end = fresh.endSession, end.id > allodia.endSessionOpened {
            allodia.endSessionOpened = end.id
            // Best-effort and unreported: this device is signed out whatever the browser does.
            // What it buys is the next sign-in asking who you are, rather than completing
            // silently against a session someone thought they had left.
            openAllodia(url: end.url)
        }
        if fresh != allodia.card { allodia.card = fresh }
        let subscription = app.allodiaSubscriptionCard()
        if subscription != allodia.subscription { allodia.subscription = subscription }
    }

    /// Opens the service's own account page. A page, not a flow: nothing is pending and nothing
    /// comes back.
    func openAllodiaAccountPage() {
        guard let url = app?.allodiaAccountUrl() else { return }
        openAllodia(url: url)
    }

    private func openAllodia(url: String) {
        let page = AllodiaPage()
        allodiaPage = page
        page.present(url: url)
    }
}

/// What Settings → Allodia account draws.
enum AllodiaCardFace: Equatable {
    /// A sign-in is out: a spinner and its Cancel, in place of every button.
    case signingIn(BrowserSignIn)
    /// Sign in and Create an account.
    case signedOut
    /// Who, Manage account and Sign out.
    case signedIn(AllodiaAccount)
    /// Whose sign-in ended, Sign in again and Sign out.
    case ended(AllodiaAccount)
}

/// What the card's record says, worded and read the same on Settings and the first-run card.
enum AllodiaCardRules {
    static func face(_ card: AllodiaCard) -> AllodiaCardFace {
        if let browser = card.browser { return .signingIn(browser) }
        return switch card.state {
        case .signedOut: .signedOut
        case let .signedIn(account): .signedIn(account)
        case let .ended(account): .ended(account)
        }
    }

    /// The first-run card's way back from a sign-in: drawn once the core says the hop has run
    /// long enough to need one, so a hop that comes straight back never puts a button in front of
    /// somebody who had no reason to read it.
    static func escape(_ card: AllodiaCard) -> BrowserSignIn? {
        guard case .signingIn(escape: true) = card.onboarding else { return nil }
        return card.browser
    }

    /// Whoever is signed in, whether or not the service still accepts the sign-in.
    static func account(_ state: AllodiaCardState?) -> AllodiaAccount? {
        switch state {
        case let .signedIn(account)?, let .ended(account)?: account
        case .signedOut?, nil: nil
        }
    }

    /// Why the last sign-in or sign-out did not work.
    static func failure(_ failure: AllodiaFailure) -> String {
        switch failure {
        case .signIn(.staleSignIn): L10n.signin_stale()
        case .signIn(.browserNotOpened): L10n.settings_allodia_browser_failed()
        case .signIn(.notFinished): L10n.settings_allodia_timeout()
        case let .signIn(refusal):
            L10n.settings_allodia_failed(error: AccountSettingsLabels.refused(refusal))
        case let .signOut(detail): L10n.settings_allodia_failed(error: detail)
        }
    }
}
