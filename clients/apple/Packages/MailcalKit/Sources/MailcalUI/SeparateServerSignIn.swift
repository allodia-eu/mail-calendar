// Signing a separate calendar or address book server in at its own provider, from the form that
// otherwise asks for its password (docs/mail-oauth.md rule 13, docs/accounts.md rule 18). The
// server is asked once the form is drawn; until it answers the form is the password form, and an
// answer never takes away a field somebody has started typing in. The way back from the browser is
// the setup sign-in's own (`SignInAttempts`, docs/onboarding.md "Nobody may be stranded on it").

import Foundation
import MailcalBindings

/// What the server answered about a sign-in at its own provider, and which form it was asked for.
struct SeparateServerOffer: Equatable {
    let account: String
    let uses: [AccountCapability]
    let offers: Bool

    init(_ ask: SeparateServerAsk, offers: Bool) {
        account = ask.account
        uses = ask.uses
        self.offers = offers
    }

    /// Whether this is the answer for `ask`: the same mailbox and the same uses.
    func answers(_ ask: SeparateServerAsk) -> Bool {
        account == ask.account && uses == ask.uses
    }
}

/// How a sign-in at the server's own provider ended.
enum SeparateServerSignInOutcome: Equatable {
    /// The uses are on an account of their own, linked to the mailbox.
    case moved
    /// The browser was dismissed, or the attempt retired. Not a failure, and nothing is said.
    case cancelled
    /// Said under the form, which stays.
    case failed(String)
}

/// What the form does once a Connect or a sign-in has answered.
enum SeparateServerNext: Equatable {
    /// The uses moved: setup goes on to the link step, and the account's page folds the form away.
    case done
    /// The form stays, saying `failure` under it when there is one.
    case stay(failure: String?)
}

extension SeparateServerRules {
    /// Where Connect leads: `refused` is the core's reason, `nil` once the uses moved.
    static func afterConnect(_ refused: String?) -> SeparateServerNext {
        refused.map { .stay(failure: $0) } ?? .done
    }

    /// Where a sign-in leads: a success where Connect's success does.
    static func afterSignIn(_ outcome: SeparateServerSignInOutcome) -> SeparateServerNext {
        switch outcome {
        case .moved: .done
        case .cancelled: .stay(failure: nil)
        case let .failed(reason): .stay(failure: reason)
        }
    }

    /// What a sign-in that did not move the uses says: the existing mapping, with an exchange or a
    /// discovery that failed read as the sign-in failing, since a password may still work.
    static func signInFailure(_ error: Error) -> String {
        if case MailcalError.Connect(_) = error { return L10n.setup_imap_signin_failed() }
        return coreMessage(error)
    }

    /// Whether the person has started on the password route: a password typed, or the login
    /// changed from the one the form started from.
    static func passwordStarted(_ ask: SeparateServerAsk, username: String, password: String)
        -> Bool
    {
        !password.isEmpty || username.trimmingCharacters(in: .whitespaces) != ask.username
    }
}

/// One separate-server form's sign-in: whether it is offered, whether the password route is drawn
/// beside it, and the attempts its button starts.
struct SeparateServerSignIn: Equatable {
    /// The server's answer; `nil` until it has given one.
    private(set) var offer: SeparateServerOffer?
    /// Whether the password route is drawn beside the sign-in, as the IMAP card's "Use a password
    /// instead" draws it.
    private(set) var passwordChosen = false
    /// The attempts the sign-in button starts. Settable so the escape clock can arm the way back.
    var attempts = SignInAttempts()
    /// Whether the attempt in flight is exchanging the redirect and adding the account, which
    /// nothing interrupts, so it draws no way back.
    private(set) var completing = false

    /// The sign-in is offered only once the server said yes, and only for the form it was asked
    /// about.
    func offersSignIn(_ ask: SeparateServerAsk) -> Bool {
        guard let offer else { return false }
        return offer.offers && offer.answers(ask)
    }

    /// The login and password: always without a sign-in; beside one, once the password route is
    /// chosen.
    func showsPasswordFields(_ ask: SeparateServerAsk) -> Bool {
        !offersSignIn(ask) || passwordChosen
    }

    /// Whether the sign-in is the form's primary action, with the password route behind "Use a
    /// password instead".
    func signInLeads(_ ask: SeparateServerAsk) -> Bool {
        offersSignIn(ask) && !passwordChosen
    }

    var signingIn: Bool { attempts.signingIn }

    /// The way back from the browser, once the hop has outlasted its threshold.
    var escapable: Bool { attempts.escapable && !completing }

    /// The server answered. `started` keeps the fields of a password route somebody has begun:
    /// an answer must not take them away.
    mutating func answered(_ offer: SeparateServerOffer, started: Bool) {
        self.offer = offer
        if started { passwordChosen = true }
    }

    mutating func choosePassword() {
        passwordChosen = true
    }

    /// Starts an attempt, superseding the one in flight, and names it.
    mutating func begin() -> Int {
        completing = false
        return attempts.begin()
    }

    /// `attempt`'s redirect arrived and the core is adding the account.
    mutating func redirected(_ attempt: Int) {
        if attempts.current == attempt { completing = true }
    }

    /// The person took a way back: the attempt in flight is retired and its answer dropped.
    mutating func retire() {
        attempts.escape()
        completing = false
    }

    /// `attempt` answered. What the form does next, or `nil` for an attempt superseded or retired,
    /// whose answer belongs to no screen. A failure brings the password route back.
    mutating func finish(_ attempt: Int, _ outcome: SeparateServerSignInOutcome)
        -> SeparateServerNext?
    {
        guard attempts.finish(attempt) else { return nil }
        completing = false
        if case .failed = outcome { passwordChosen = true }
        return SeparateServerRules.afterSignIn(outcome)
    }
}
