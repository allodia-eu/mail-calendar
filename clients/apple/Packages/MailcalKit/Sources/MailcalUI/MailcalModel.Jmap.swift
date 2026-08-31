// The JMAP side of the model: the OAuth sign-in ("Sign in with your provider") and the one
// add + store path every JMAP account lands through. Factored into its own `MailboxModel`
// extension the way MailcalModel.Google.swift is, so MailcalModel.swift stays readable.
//
// The core owns the OAuth/token lifecycle and the discovery that precedes it; this host owns the
// browser hop (`JmapSignIn`) and the Keychain. Sign-in is an *addition* to the password/API-token
// field, never a replacement, discovery can decline on any server, so both routes stay live and
// both end in the same `addAndStoreJmapAccount`.

import Foundation
import MailcalBindings

/// How a JMAP sign-in ended. The setup form has to tell a real failure (show the "use a password
/// or API token instead" line) from the user simply closing the browser (say nothing), a
/// dismissed browser is not an error, exactly as in the Microsoft and Google flows.
enum JmapSignInOutcome {
    case connected
    case cancelled
    case failed
}

extension MailboxModel {
    /// Whether this server advertises discoverable OAuth sign-in, so the form can decide whether
    /// to *show* the button rather than offering one that dead-ends. **Blocking** in the core (it
    /// probes the server), so it runs off the main thread, the setup form debounces it on the
    /// email field. Never throws: anything short of a usable answer is `false`.
    func jmapOAuthAvailable(email: String, serverURL: String) async -> Bool {
        guard let app else { return false }
        let server = serverURL.isEmpty ? nil : serverURL
        return await Task.detached(priority: .userInitiated) {
            app.jmapOauthAvailable(email: email, serverUrl: server)
        }.value
    }

    /// Runs a JMAP OAuth sign-in: the core discovers the authorization server and registers this
    /// install, we open the returned URL in the browser (ASWebAuthenticationSession), and the core
    /// exchanges the redirect for the `[jmap]` config TOML. That TOML is the same shape the manual
    /// form produces, so it is added and stored through the same path.
    func signInWithJmap(email: String, serverURL: String) async -> JmapSignInOutcome {
        guard let app else {
            setupError = "Could not open the app. Please relaunch."
            return .failed
        }
        let server = serverURL.isEmpty ? nil : serverURL
        // The session's `presentationContextProvider` is a WEAK reference, so this local is what
        // keeps the provider alive; the awaited `authorize` call spans the whole presentation, so
        // it cannot be released while the browser is up.
        let signIn = JmapSignIn()
        do {
            // Discovery + dynamic client registration are several network round trips and the core
            // call blocks on them, off the main thread, or the whole UI freezes.
            let start = try await Task.detached(priority: .userInitiated) {
                try app.beginJmapLogin(
                    email: email, serverUrl: server, redirectUri: JmapOAuthConfig.redirectURI)
            }.value
            // ASWebAuthenticationSession must start on the main actor.
            let callbackURL = try await signIn.authorize(authorizationURL: start.authorizationUrl)
            // The token exchange is blocking too, as is the connect + first sync below.
            let configToml = try await Task.detached(priority: .userInitiated) {
                try app.completeJmapLogin(pending: start.pending, callbackUrl: callbackURL)
            }.value
            try await addAndStoreJmapAccount(app, configToml: configToml)
            setupError = nil
            return .connected
        } catch JmapSignInError.cancelled {
            // The user dismissed the browser, not an error, and nothing to say about it.
            return .cancelled
        } catch {
            // The form shows the localised "you can use a password or API token instead" line; the
            // specific cause goes to the diagnostic log instead of in front of the user. The core's
            // messages name server configuration (an issuer, an endpoint), never the address or a
            // secret, so this stays inside the never-log-content rule.
            logAppleLifecycle("jmap oauth: sign-in did not complete (\(error))")
            return .failed
        }
    }

    /// Signs an existing OAuth JMAP account back in, from the expired-sign-in banner. The core
    /// builds the authorization URL from that account's own persisted grant (no re-discovery, no
    /// second client registration), we run the same browser hop, and the core swaps the fresh
    /// grant in, connect, Keychain, and the retracted prompt included. So unlike a first sign-in
    /// there is no `addAccount` and no `KeychainStore.save` here: the account already exists.
    func reconnectJmap(accountId: String) async {
        guard let app, !jmapReconnecting else { return }
        // A weak `presentationContextProvider`, so this local is what keeps it alive for the
        // duration of the awaited presentation, the same reason `signInWithJmap` holds one.
        let signIn = JmapSignIn()
        jmapReconnecting = true
        defer { jmapReconnecting = false }
        do {
            let start = try await Task.detached(priority: .userInitiated) {
                try app.beginJmapReauth(accountId: accountId)
            }.value
            // ASWebAuthenticationSession must start on the main actor.
            let callbackURL = try await signIn.authorize(authorizationURL: start.authorizationUrl)
            // The exchange, the connect and the catch-up sync all block.
            try await Task.detached(priority: .userInitiated) {
                try app.completeJmapReauth(
                    accountId: accountId, pending: start.pending, callbackUrl: callbackURL)
            }.value
            accountNotice = nil
        } catch JmapSignInError.cancelled {
            // The user dismissed the browser. The banner is still there to try again from.
        } catch {
            // Say so rather than leaving the tap looking like it did nothing, in plain words,
            // because the cause is an OAuth protocol string. The banner stays up (the core leaves
            // the prompt raised on every failure), so the remedy is still one tap away, and the
            // specific cause goes to the diagnostic log: the core's messages name endpoints and
            // error codes, never the address or a secret.
            logAppleLifecycle("jmap oauth: re-authentication did not complete (\(error))")
            accountNotice = L10n.signin_expired_failed()
        }
    }

    /// Connects `configToml` as a new account. The single add path for JMAP: the manual form's
    /// secret and an OAuth grant differ only in what the TOML carries.
    ///
    /// The Keychain write is the core's, inside `addAccount`, which is also the only place that
    /// can see a refresh-token rotation landing *during* the connect, and so the only place that
    /// can store the grant the server actually ended up with.
    ///
    /// `addAccount` blocks on the JMAP connect + first sync, so it runs off the main thread.
    func addAndStoreJmapAccount(_ app: MailcalApp, configToml: String) async throws {
        _ = try await Task.detached(priority: .userInitiated) {
            try app.addAccount(configToml: configToml)
        }.value
        accountWasAdded()
    }
}
