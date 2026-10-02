// The IMAP side of the model: asking a mail server what it accepts, and the browser sign-in when
// the answer is "sign in". The sibling of MailcalModel.Jmap.swift, and the same division of
// labour: the core owns discovery, PKCE, the exchange and the token lifecycle, this host owns the
// browser hop (ASWebAuthenticationSession) and the Keychain.
//
// One thing differs from the JMAP flow, and it is why this exists rather than a shared function.
// A JMAP server is an HTTP resource, so an unauthenticated request answers 401 and names its
// authorization server. An IMAP server has no such surface, so the core asks the *mail server*
// what it accepts first and the answer has three shapes rather than two (docs/mail-oauth.md).

import Foundation
import MailcalBindings

/// How an IMAP sign-in ended. The form has to tell a real failure (show the "use a password
/// instead" line) from the person simply closing the browser (say nothing): a dismissed browser is
/// not an error, exactly as in the Microsoft, Google and JMAP flows.
enum ImapSignInOutcome {
    case connected
    case cancelled
    case failed
}

extension MailboxModel {
    /// What this mail server accepts, so the form can decide what to ask for rather than guessing.
    ///
    /// **Blocking** in the core (it dials the mail server, then may fetch metadata), so it runs off
    /// the main thread. Never throws: anything short of a usable answer is `.password`, which is
    /// what works everywhere.
    func imapAuthOptions(_ request: ImapLoginRequest) async -> ImapAuthOffer {
        guard let app else { return .password }
        return await Task.detached(priority: .userInitiated) {
            app.imapAuthOptions(request: request)
        }.value
    }

    /// Runs an IMAP OAuth sign-in: the core finds the authorization server and registers this
    /// install, we open the returned URL in the browser, and the core exchanges the redirect for
    /// the account-config TOML. That TOML is the same shape the password form produces, so it is
    /// added and stored through the path that already exists.
    func signInWithImap(_ request: ImapLoginRequest) async -> ImapSignInOutcome {
        guard let app else {
            setupError = "Could not open the app. Please relaunch."
            return .failed
        }
        // The session's `presentationContextProvider` is a WEAK reference, so this local is what
        // keeps the provider alive; the awaited `authorize` call spans the whole presentation, so
        // it cannot be released while the browser is up.
        let signIn = JmapSignIn()
        do {
            // Discovery and dynamic client registration are several network round trips and the
            // core call blocks on them: off the main thread, or the whole UI freezes.
            let start = try await Task.detached(priority: .userInitiated) {
                try app.beginImapLogin(request: request, redirectUri: ImapOAuthConfig.redirectURI)
            }.value
            // ASWebAuthenticationSession must start on the main actor.
            let callbackURL = try await signIn.authorize(authorizationURL: start.authorizationUrl)
            // The token exchange blocks too, as does the connect and first sync below. The add
            // is the JMAP path's, which is not JMAP-specific: it is the one place a refresh-token
            // rotation landing *during* the connect can still be stored, and every OAuth account
            // needs that whichever protocol it speaks.
            let configToml = try await Task.detached(priority: .userInitiated) {
                try app.completeImapLogin(pending: start.pending, callbackUrl: callbackURL)
            }.value
            try await addAndStoreJmapAccount(app, configToml: configToml)
            setupError = nil
            return .connected
        } catch JmapSignInError.cancelled {
            // The person dismissed the browser. Not an error, and nothing to say about it.
            return .cancelled
        } catch {
            // The form shows the localised "you can use a password instead" line; the specific
            // cause goes to the diagnostic log rather than in front of the user. The core's
            // messages name server configuration (an issuer, an endpoint), never the address or a
            // secret, so this stays inside the never-log-content rule.
            logAppleLifecycle("imap oauth: sign-in did not complete (\(error))")
            return .failed
        }
    }
}

/// Where an IMAP authorization server sends the browser back to.
///
/// Its own scheme rather than the JMAP one, because the value rides through to the token endpoint
/// verbatim on every later refresh: two flows sharing one would be indistinguishable in a
/// provider's own record of what it registered.
enum ImapOAuthConfig {
    static let redirectURI = "\(Brand.appID)://imap-oauth"
}
