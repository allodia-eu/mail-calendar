// A mailbox's calendar and contacts on a server its sign-in does not reach (docs/accounts.md rule
// 18): moved to an account of their own with a password typed for that server, or signed in at
// that server's own provider (docs/mail-oauth.md rule 13), or switched off. The form draws the
// core's section, and the password and Skip are changes on the account's page. The core adds and
// connects the calendar account before the mailbox changes, so a refusal leaves both as they were.
// Every sign-in call blocks in the core, so each runs off the main actor.

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

    /// Reads every open form again: a Connect or Skip starting or ending, or the server's answer
    /// about its ways in. One the core draws no section for is over: the uses have an account of
    /// their own, or were switched off, so the person's other devices hear of it and setup goes on.
    func refreshSeparateServers() {
        for (place, shown) in separateServers {
            let fresh = separateServerAsk(shown.account)
            guard fresh != shown else { continue }
            separateServers[place] = fresh
            if fresh == nil {
                syncAfterAccountChange()
                if place == .setup { setupSeparateServerDone() }
            }
        }
    }

    /// Moves `ask`'s uses to an account of their own signed in at their server's own provider.
    /// The core finds the issuer, registers this install and builds the URL; this host opens it
    /// and hands the redirect back. `redirected` is told once the exchange and the add begin.
    ///
    /// Cancelling the calling task retires the attempt: neither blocking leg can be interrupted, so
    /// each is followed by the check that keeps a retired attempt from opening a browser or adding
    /// an account (docs/onboarding.md, "Nobody may be stranded on it").
    func signInSeparateServer(
        _ ask: SeparateServerAsk, redirected: () -> Void
    ) async -> SeparateServerSignInOutcome {
        guard let app else { return .failed(L10n.app_unavailable()) }
        // The session's presentation provider is a weak reference, so this local keeps it alive
        // for the whole awaited presentation.
        let browser = JmapSignIn()
        do {
            let start = try await Task.detached(priority: .userInitiated) {
                try app.beginSeparateServerSignIn(
                    accountId: ask.account, uses: ask.uses,
                    redirectUri: ImapOAuthConfig.redirectURI)
            }.value
            if Task.isCancelled { return .cancelled }
            // ASWebAuthenticationSession must start on the main actor. It catches this redirect as
            // it does an IMAP sign-in's, which shares the app's scheme.
            let callbackURL = try await browser.authorize(authorizationURL: start.authorizationUrl)
            if Task.isCancelled { return .cancelled }
            redirected()
            _ = try await Task.detached(priority: .userInitiated) {
                try app.completeSeparateServerSignIn(
                    accountId: ask.account, pending: start.pending, callbackUrl: callbackURL)
            }.value
            logBrowserSignIn("calendar", "the separate server was signed in at its own provider")
            // An account was added, so it travels as every added account does.
            syncAfterAccountChange()
            refreshAccountsSnapshot()
            return .moved
        } catch JmapSignInError.cancelled {
            return .cancelled
        } catch JmapSignInError.badURL, JmapSignInError.couldNotStart {
            return .failed(L10n.setup_imap_signin_failed())
        } catch {
            // The core's messages name server configuration, never the login or a secret.
            logBrowserSignIn("calendar", "signing in did not complete (\(coreMessage(error)))")
            return .failed(SeparateServerRules.signInFailure(error))
        }
    }
}
