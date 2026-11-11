// A mailbox's calendar and contacts on a server its sign-in does not reach (docs/accounts.md rule
// 18): moved to an account of their own with a password typed for that server, or signed in at
// that server's own provider (docs/mail-oauth.md rule 13) or with Nextcloud, or switched off. The
// form draws the core's section; the password, Skip and the browser sign-in are changes on the
// account's page. The core adds and connects the calendar account before the mailbox changes, so
// a refusal leaves both as they were.

import Foundation
import MailcalBindings

/// Where a separate-server form is open.
enum SeparateServerPlace: Hashable {
    /// Setup's step after the add.
    case setup
    /// The form on an account's page.
    case page
}

extension MailboxModel {
    /// The form for `account`'s separate server, from the core's page; `nil` when nothing needs
    /// one.
    func separateServerAsk(_ account: String) -> SeparateServerAsk? {
        SeparateServerRules.ask(account, app?.accountPage(accountId: account)?.separateServer)
    }

    /// Opens the page's form for `account`.
    func openSeparateServer(_ account: String) {
        separateServers[.page] = separateServerAsk(account)
    }

    /// The form open on `account`'s page, as the core last drew it.
    func pageSeparateServer(_ account: String) -> SeparateServerAsk? {
        separateServers[.page].flatMap { $0.account == account ? $0 : nil }
    }

    /// Reads every open form again: a Connect, Skip or browser sign-in starting or ending, or the
    /// server's answer about its ways in. One the core draws no section for is over: the uses
    /// have an account of their own, or were switched off, and setup goes on.
    func refreshSeparateServers() {
        for (place, shown) in separateServers {
            let fresh = separateServerAsk(shown.account)
            guard fresh != shown else { continue }
            separateServers[place] = fresh
            if fresh == nil, place == .setup { setupSeparateServerDone() }
        }
    }

    /// Signs `account`'s separate server in through the core's browser sign-in, the way the
    /// server offers, superseding one still out. A provider returns to the redirect an IMAP
    /// account's sign-in does, which the auth session catches; the core ignores it for
    /// Nextcloud, which returns nothing.
    func signInSeparateServer(_ account: String) {
        changeAccount(
            .signInSeparateServer(account: account, redirectUri: ImapOAuthConfig.redirectURI))
        driveBrowserSignIn()
    }
}
