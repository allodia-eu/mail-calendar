// A mailbox's calendar and contacts on a server its sign-in does not reach (docs/accounts.md rule
// 18): moved to an account of their own with a password typed for that server, or signed in at
// that server's own provider (docs/mail-oauth.md rule 13), or switched off.
// The core adds and connects the calendar account before the mailbox changes, so a refusal leaves
// both as they were. Every call blocks in the core, so each runs off the main actor.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Moves `ask`'s uses to an account of their own at the same server, signing in as
    /// `username` with `password`, and links it to the mailbox. Answers why not, or `nil` once
    /// moved; the page and the link step then redraw from the next snapshot.
    func addLinkedCalendarAccount(
        _ ask: SeparateServerAsk, username: String, password: String
    ) async -> String? {
        guard let app else { return L10n.app_unavailable() }
        do {
            _ = try await Task.detached(priority: .userInitiated) {
                try app.addLinkedCalendarAccount(
                    accountId: ask.account, uses: ask.uses, username: username, password: password)
            }.value
            // An account was added, so it travels as every added account does.
            syncAfterAccountChange()
            refreshAccountsSnapshot()
            return nil
        } catch {
            // Never the login or the password.
            logAppleLifecycle(
                "accounts: the calendar server's own sign-in failed (\(coreMessage(error)))")
            return coreMessage(error)
        }
    }

    /// Whether the server holding `ask`'s uses can be signed in to at its own provider. Never
    /// fails: anything short of a usable answer is the password form.
    func separateServerOffersSignIn(_ ask: SeparateServerAsk) async -> Bool {
        guard let app else { return false }
        return await Task.detached(priority: .userInitiated) {
            app.separateServerOffersSignIn(accountId: ask.account, uses: ask.uses)
        }.value
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

    /// Skip on the setup step: the uses the sign-in did not reach are switched off, so the account
    /// does not open on a calendar that cannot connect. Its page offers the same form later.
    func skipSeparateServer(_ ask: SeparateServerAsk) async {
        // A failure is logged by the switch itself, and setup goes on: the page still offers it.
        for use in SeparateServerRules.skipped(ask) {
            _ = await setAccountUse(ask.account, use, on: false)
        }
    }
}
