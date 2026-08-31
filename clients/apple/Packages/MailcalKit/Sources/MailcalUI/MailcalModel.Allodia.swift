// The Allodia-account side of the model: the browser sign-in, who is signed in, and signing out.
// Factored into its own `MailboxModel` extension the way MailcalModel.Jmap.swift is.
//
// The core owns discovery, PKCE, the exchange and the Keychain write; this host owns the browser
// hop (`AllodiaSignIn`) and nothing else, in particular it never sees or stores a token.

import Foundation
import MailcalBindings

/// How a sign-in ended. A dismissed browser is not a failure and says nothing, exactly as in the
/// Microsoft, Google and JMAP flows.
enum AllodiaSignInOutcome {
    case signedIn(AllodiaAccount)
    case cancelled
    case failed(String)
}

extension MailboxModel {
    /// Who is signed in to an Allodia account, or `nil`. Cheap and local, it reads what the last
    /// launch restored or the last sign-in wrote, and never asks the service.
    func currentAllodiaAccount() -> AllodiaAccount? {
        app?.allodiaAccount()
    }

    /// Runs a sign-in: the core reads the service's OAuth metadata and mints the authorization URL,
    /// we open it in the browser, and the core exchanges the redirect, asks whose account it is and
    /// stores the grant itself.
    ///
    /// Cancelling the calling task **retires the attempt**, which is more than stopping the wait:
    /// neither of the two blocking legs can be interrupted, so each is followed by the check that
    /// keeps a finished one from opening a browser, or storing a grant, for a sign-in somebody has
    /// already escaped ([`docs/onboarding.md`](docs/onboarding.md)).
    func signInToAllodia(create: Bool = false) async -> AllodiaSignInOutcome {
        guard let app else {
            return .failed("Could not open the app. Please relaunch.")
        }
        // The session's `presentationContextProvider` is a WEAK reference, so this local is what
        // keeps the provider alive; the awaited `authorize` call spans the whole presentation.
        let signIn = AllodiaSignIn()
        do {
            // Discovery is two network round trips and the core call blocks on them, off the main
            // thread, or the whole UI freezes.
            let start = try await Task.detached(priority: .userInitiated) {
                create
                    ? try app.beginAllodiaRegistration(redirectUri: AllodiaOAuthConfig.redirectURI)
                    : try app.beginAllodiaSignIn(redirectUri: AllodiaOAuthConfig.redirectURI)
            }.value
            // A detached task runs to its end whatever the caller does, so the read that just
            // finished may be one the person gave up on. This is where such an attempt stops:
            // opening a browser for it is exactly what the way out has to prevent.
            if Task.isCancelled { return .cancelled }
            // ASWebAuthenticationSession must start on the main actor.
            let callbackURL = try await signIn.authorize(authorizationURL: start.authorizationUrl)
            // A redirect can arrive in the same moment somebody escapes. The grant is the half that
            // would outlive the screen, so it is not stored for an attempt already given up on.
            if Task.isCancelled { return .cancelled }
            // The exchange and the identity lookup block too.
            let account = try await Task.detached(priority: .userInitiated) {
                try app.completeAllodiaSignIn(pending: start.pending, callbackUrl: callbackURL)
            }.value
            return .signedIn(account)
        } catch AllodiaSignInError.cancelled {
            // The user dismissed the browser, not an error, and nothing to say about it.
            return .cancelled
        } catch {
            // The core's messages name endpoints, scopes and error codes, never an address or a
            // secret, so showing and logging one stays inside the never-log-content rule.
            logAppleLifecycle("allodia: sign-in did not complete (\(error))")
            return .failed("\(error)")
        }
    }

    /// Signs out: forgets the account and erases its stored grant. Returns the error text when the
    /// platform store refused the delete, and `nil` on success.
    ///
    /// Local only, which is what removing a mail account is too: the grant stays alive at the
    /// service until it expires or the person revokes it there.
    func signOutOfAllodia() -> String? {
        guard let app else { return nil }
        do {
            // Best-effort and deliberately unreported: this device is signed out whatever happens
            // to the browser. What opening it buys is the next sign-in asking who you are, rather
            // than completing silently against a session someone thought they had left.
            if let endSession = try app.signOutOfAllodia() {
                openAllodia(url: endSession)
            }

            return nil
        } catch {
            logAppleLifecycle("allodia: sign-out did not complete (\(error))")
            return "\(error)"
        }
    }

    /// Opens the service's own account page. A page, not a flow: nothing is pending and nothing
    /// comes back.
    func openAllodiaAccountPage() {
        guard let url = app?.allodiaAccountUrl() else { return }
        openAllodia(url: url)
    }

    /// Presents `url` in the in-app browser tab, holding the session alive for its lifetime, the
    /// presentation context provider is a **weak** reference, so a released one leaves the browser
    /// with nowhere to present.
    private func openAllodia(url: String) {
        let browser = AllodiaSignIn()
        allodiaBrowser = browser
        browser.present(url: url)
    }
}
