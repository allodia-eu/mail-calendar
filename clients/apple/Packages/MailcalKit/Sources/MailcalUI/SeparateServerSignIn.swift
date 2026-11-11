// Signing a separate calendar or address book server in through the browser, from the form that
// otherwise asks for its password (docs/mail-oauth.md rule 13, docs/accounts.md rule 18): at the
// server's own provider, or with Nextcloud. The core asks the server and draws its answer on the
// section, and runs the sign-in itself (`AccountSeparateServer.browser`); until the server has
// answered, the form is the password form, and an answer never takes away a field somebody has
// started typing in.

import Foundation
import MailcalBindings

extension SeparateServerRules {
    /// Whether the person has started on the password route: a password typed, or the login
    /// changed from the one the form started from.
    static func passwordStarted(_ ask: SeparateServerAsk, username: String, password: String)
        -> Bool
    {
        !password.isEmpty || username.trimmingCharacters(in: .whitespaces) != ask.username
    }

    /// The sign-in the browser holds for `section` while it waits on the person, with its Cancel;
    /// `nil` while the core starts or completes it, and while none is out.
    static func waiting(_ section: AccountSeparateServer) -> BrowserSignIn? {
        guard let browser = section.browser, case .inBrowser = browser.stage else { return nil }
        return browser
    }
}

/// One separate-server form's sign-in: whether it is offered, and whether the password route is
/// drawn beside it.
struct SeparateServerSignIn: Equatable {
    /// Whether the password route is drawn beside the sign-in, as the IMAP card's "Use a password
    /// instead" draws it.
    private(set) var passwordChosen = false

    /// The way in the server offers in the browser, once the core draws it.
    func offers(_ ask: SeparateServerAsk) -> AccountSeparateSignIn? {
        ask.section.signIn
    }

    /// The login and password: always without a sign-in; beside one, once the password route is
    /// chosen.
    func showsPasswordFields(_ ask: SeparateServerAsk) -> Bool {
        offers(ask) == nil || passwordChosen
    }

    /// Whether the sign-in is the form's primary action, with the password route behind "Use a
    /// password instead".
    func signInLeads(_ ask: SeparateServerAsk) -> Bool {
        offers(ask) != nil && !passwordChosen
    }

    /// The core drew `fresh` over `shown`. A way in it now offers keeps the fields of a password
    /// route somebody has `started`, and a failure brings the password route back, since a
    /// password may still work where the browser did not.
    mutating func drawn(
        _ shown: AccountSeparateServer, _ fresh: AccountSeparateServer, started: Bool
    ) {
        if shown.signIn == nil, fresh.signIn != nil, started { passwordChosen = true }
        if fresh.failed != nil { passwordChosen = true }
    }

    mutating func choosePassword() {
        passwordChosen = true
    }
}

extension SeparateServerLabels {
    /// The sign-in's button.
    static func signInButton(_ way: AccountSeparateSignIn) -> String {
        switch way {
        case .provider: L10n.setup_imap_signin_button()
        case .nextcloud: L10n.nextcloud_sign_in_button()
        }
    }

    /// The line under the sign-in: while the browser holds it, that it waits there, named by the
    /// server's login; otherwise whose sign-in it is.
    static func signInLine(_ way: AccountSeparateSignIn, _ section: AccountSeparateServer)
        -> String
    {
        AccountSettingsLabels.signInWaiting(section.browser, name: section.username)
            ?? signInNote(way)
    }

    private static func signInNote(_ way: AccountSeparateSignIn) -> String {
        switch way {
        case .provider: L10n.setup_imap_signin_note()
        case .nextcloud: L10n.nextcloud_sign_in_note()
        }
    }
}
