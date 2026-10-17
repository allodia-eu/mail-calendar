// Switching a standards or JMAP account between a password and signing in through its provider,
// and signing one in again in place (docs/accounts.md rule 17). The core tests the new credential,
// stores it and replaces the account; this host owns only the browser hop. Every call blocks in
// the core, so each runs off the main actor.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Which ways in `id`'s server takes. Blocking in the core (detection, a capability probe and
    /// issuer discovery); never fails, and `nil` only with no core to ask.
    func accountSignInOptions(_ id: String) async -> AccountSignInOptions? {
        guard let app else { return nil }
        return await Task.detached(priority: .utility) {
            app.accountSignInOptions(accountId: id)
        }.value
    }

    /// Signs `id` in through its provider: again, from its stored grant, or for the first time in
    /// place of its password. The account keeps its id, uses, links, settings and mail.
    ///
    /// Not guarded against a second press while the browser is up: a newer request ends the
    /// browser of the one before it (docs/provider-oauth.md rule 13), as a Microsoft or Google
    /// sign-in in place does. The core's own work is guarded by `signInSwitchBusy`.
    func signInThroughProvider(_ id: String, redirectURI: String) async -> ConsentEnded {
        guard let app else { return .failed(L10n.app_unavailable()) }
        consentBrowser?.cancel()
        consentAttempt &+= 1
        let attempt = consentAttempt
        // The session's presentation provider is a weak reference, so this local keeps it alive
        // for the whole awaited presentation.
        let browser = JmapSignIn()
        consentBrowser = .standards(browser)
        // The newest sign-in in place owns every marker, including one the sign-in it just ended
        // was holding.
        signingInAgain = nil
        signInSwitchBusy = id
        do {
            // Discovery and registration for a password account: several round trips.
            let start = try await Task.detached(priority: .userInitiated) {
                try app.beginAccountSignIn(accountId: id, redirectUri: redirectURI)
            }.value
            guard consentAttempt == attempt else { return .cancelled }
            signInSwitchBusy = nil
            signingInAgain = id
            // ASWebAuthenticationSession must start on the main actor. It catches the redirect
            // of both an IMAP and a JMAP grant, which share the app's scheme.
            let callbackURL = try await browser.authorize(authorizationURL: start.authorizationUrl)
            guard consentAttempt == attempt else { return .cancelled }
            signingInAgain = nil
            signInSwitchBusy = id
            // The exchange, the connect and the store all block.
            try await Task.detached(priority: .userInitiated) {
                try app.completeAccountSignIn(
                    accountId: id, pending: start.pending, callbackUrl: callbackURL)
            }.value
            logBrowserSignIn("account", "the account was signed in through its provider in place")
            endSignInSwitch(attempt)
            refreshAccountsSnapshot()
            return .signedIn
        } catch JmapSignInError.cancelled {
            endSignInSwitch(attempt)
            return .cancelled
        } catch JmapSignInError.badURL, JmapSignInError.couldNotStart {
            endSignInSwitch(attempt)
            return .failed(L10n.account_signin_browser_failed())
        } catch {
            logBrowserSignIn("account", "signing in did not complete (\(coreMessage(error)))")
            endSignInSwitch(attempt)
            return .failed(coreMessage(error))
        }
    }

    /// Clears the in-flight markers, unless a newer request has taken them.
    private func endSignInSwitch(_ attempt: Int) {
        guard consentAttempt == attempt else { return }
        signInSwitchBusy = nil
        signingInAgain = nil
        consentBrowser = nil
    }

    /// Switches `id` to signing in with `password`, after proving it connects. Answers why not,
    /// or `nil` once switched.
    func switchAccountToPassword(_ id: String, _ password: String) async -> String? {
        guard let app else { return L10n.app_unavailable() }
        signInSwitchBusy = id
        defer { if signInSwitchBusy == id { signInSwitchBusy = nil } }
        do {
            try await Task.detached(priority: .userInitiated) {
                try app.switchAccountToPassword(accountId: id, password: password)
            }.value
            refreshAccountsSnapshot()
            return nil
        } catch {
            // Never the password itself.
            logAppleLifecycle("accounts: switching to a password failed (\(coreMessage(error)))")
            return coreMessage(error)
        }
    }

    /// Signs an IMAP account signed in through its provider in again, from the expired-sign-in
    /// banner. A success says nothing, because the banner clearing is the answer.
    func signInThroughProviderFromBanner(_ id: String) {
        Task {
            let ended = await signInThroughProvider(id, redirectURI: ImapOAuthConfig.redirectURI)
            if case .failed = ended {
                accountNotice = L10n.signin_expired_failed()
            }
        }
    }

    // MARK: An account already set up

    /// Keeps what the refusal of an add needs to offer updating the account already there.
    func keepExistingAccountOffer(_ error: Error, configToml: String) {
        setupExistingAccount = ExistingAccountUpdate(refusal: error, configToml: configToml)
    }

    /// Puts the credential the refused add was given on the account already there, and ends
    /// setup as an add does. The account is already in every list, so nothing is added and no
    /// name is asked.
    func updateExistingAccount() {
        guard let app, let offer = setupExistingAccount, !updatingExistingAccount else { return }
        updatingExistingAccount = true
        Task { @MainActor in
            defer { self.updatingExistingAccount = false }
            do {
                try await Task.detached(priority: .userInitiated) {
                    try app.useCredentialFrom(
                        accountId: offer.accountId, configToml: offer.configToml)
                }.value
                self.setupExistingAccount = nil
                self.accountWasAdded()
            } catch {
                logAppleLifecycle("setup: updating the account already set up failed (\(coreMessage(error)))")
                self.setupExistingAccount = nil
                self.setupError = setupFailureMessage(error)
            }
        }
    }
}
