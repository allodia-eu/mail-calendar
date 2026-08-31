// The Google sign-in flow on the model, factored into its own `MailboxModel` extension to keep
// MailcalModel.swift under the 500-line limit (the model is already split this way, see
// MailcalModel.Actions.swift, MailcalModel.Composer.swift, …). It is the exact sibling of
// `signInWithMicrosoft` (which stays in MailcalModel.swift): the Rust core owns the OAuth/token
// lifecycle; this host owns only the browser half via `GoogleSignIn` and persists the returned
// config to the Keychain.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Runs the Google sign-in: asks the core for the authorization URL, opens it in the browser
    /// (ASWebAuthenticationSession, reusing the browser's Google session), then completes the
    /// flow (token exchange + connect) and stores the returned config in the Keychain. A cancel or
    /// failure surfaces as `setupError` and keeps the form up. The Early Access confirmation (the
    /// setup UI's mandatory checkbox) gates the button, so this is only ever reached once the user
    /// has confirmed. Google grants Gmail + Calendar scopes at connect, so there is no calendar
    /// reauth follow-up.
    func signInWithGoogle(loginHint: String? = nil) {
        guard let app else {
            setupError = "Could not open the app. Please relaunch."
            return
        }
        // The platform's browser half: iOS an `ASWebAuthenticationSession` with a custom-scheme
        // redirect, macOS a loopback listener. Both are driven identically through the protocol.
        let flow = makeGoogleBrowserFlow()
        googleBrowserFlow = flow
        self.googleSigningIn = true
        Task { @MainActor in
            defer {
                self.googleBrowserFlow = nil
                self.googleSigningIn = false
            }
            do {
                // The redirect URI the core builds the authorization URL against: iOS's static
                // custom scheme, or macOS's freshly-bound `http://127.0.0.1:<port>/` loopback.
                let redirectURI = try await flow.redirectURI()
                let start = try beginGoogleLogin(
                    redirectUri: redirectURI,
                    // The address the user is connecting (from autodetection), so Google targets
                    // that account instead of a different signed-in one; nil ⇒ the account picker.
                    loginHint: loginHint.flatMap { $0.isEmpty ? nil : $0 }
                )
                // Opens the browser and awaits the redirect (must start on the main actor).
                let callbackURL = try await flow.authorize(
                    authorizationURL: start.authorizationUrl)
                // The token exchange + folder connect + first mailbox sync are blocking and can
                // take a while, so run them OFF the main thread, otherwise the whole UI freezes
                // for the duration. Hop back to the main actor for the UI + Keychain.
                _ = try await Task.detached(priority: .userInitiated) {
                    try app.completeGoogleLogin(
                        pending: start.pending, callbackUrl: callbackURL)
                }.value
                self.accountWasAdded()
            } catch GoogleSignInError.cancelled {
                // The user dismissed the browser, not an error; the defer resets the spinner.
            } catch {
                self.setupError = "\(error)"
            }
        }
    }
}
