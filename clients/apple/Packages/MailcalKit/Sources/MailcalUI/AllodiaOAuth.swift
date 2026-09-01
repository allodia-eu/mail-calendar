// Signing in to an **Allodia account**, the browser half, and the sibling of JmapOAuth.swift and
// MicrosoftOAuth.swift. As there, the Rust core owns the whole OAuth state machine (discovery,
// PKCE, the exchange, the Keychain write) and this host owns only opening the authorization URL
// and capturing the redirect through ASWebAuthenticationSession, which reuses the system browser's
// session.
//
// An Allodia account is not a mail account: it carries no mailbox, appears in no switcher, and a
// token issued for it cannot touch anyone's mail. Its screen is Settings → Accounts, never the
// setup wizard.
//
// The route exists only in a build Allodia ships. `allodiaSignInAvailable()` answers `false`
// otherwise, the ordinary answer for a build from source, and the settings screen then draws
// nothing at all rather than a button that dead-ends.

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import AuthenticationServices
import Foundation
import MailcalBindings

/// The one piece of OAuth configuration this client owns: where the account service sends the
/// browser back to.
enum AllodiaOAuthConfig {
    /// A custom scheme on the app's own bundle identifier, registered against the static client
    /// registration the build was given. Nothing declares it in the Info.plist:
    /// `ASWebAuthenticationSession` claims the scheme for the life of the session, exactly as the
    /// Microsoft and JMAP flows here already rely on.
    ///
    /// The `account-oauth` host is deliberately not `auth` or `jmap-oauth`: Windows and Android
    /// dispatch an arriving redirect by that label, and two flows sharing one is a redirect
    /// delivered to the wrong flow, which fails by never coming back rather than by erroring.
    static let redirectURI = "\(Brand.appID)://account-oauth"

    /// The scheme part of `redirectURI` (before `://`), which ASWebAuthenticationSession watches
    /// for to know the flow is done.
    static var callbackScheme: String {
        String(redirectURI.prefix { $0 != ":" })
    }
}

/// A failure in the browser half of the flow.
enum AllodiaSignInError: LocalizedError {
    case badURL
    case cancelled
    case couldNotStart

    var errorDescription: String? {
        switch self {
        case .badURL: return "The sign-in URL was invalid."
        case .cancelled: return "Sign-in was cancelled."
        case .couldNotStart: return "Could not start sign-in."
        }
    }
}

/// Drives one ASWebAuthenticationSession sign-in: opens the authorization URL in the system
/// browser and resolves with the redirect callback URL. The shape of `MicrosoftSignIn` and
/// `JmapSignIn`, the three differ only in which scheme they wait for.
///
/// The caller must hold this alive for the whole flow: the session's `presentationContextProvider`
/// is a **weak** reference, so a released provider leaves the browser with nowhere to present.
@MainActor
final class AllodiaSignIn: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    /// Opens `authorizationURL` and awaits the redirect, returning the full callback URL.
    /// Throws on user cancel or a session error.
    ///
    /// Cancelling the awaiting task ends the presentation rather than merely stopping the wait: a
    /// session left running would put its redirect into a sign-in nobody is waiting for any more,
    /// and on macOS would leave a browser window open over a screen that has moved on.
    func authorize(authorizationURL: String) async throws -> String {
        guard let url = URL(string: authorizationURL) else {
            throw AllodiaSignInError.badURL
        }
        return try await withTaskCancellationHandler {
            // Cancelled before the session started: `onCancel` had no session to end, so nothing
            // else would ever stop this one.
            if Task.isCancelled { throw AllodiaSignInError.cancelled }
            return try await presentAndWait(url: url)
        } onCancel: {
            Task { @MainActor in self.cancel() }
        }
    }

    /// Ends the presentation. The session reports a cancelled login, so the awaiting `authorize`
    /// returns through the ordinary "they closed it" path and nothing else has to know.
    func cancel() {
        session?.cancel()
    }

    private func presentAndWait(url: URL) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: url,
                callbackURLScheme: AllodiaOAuthConfig.callbackScheme
            ) { @Sendable callbackURL, error in
                if let callbackURL {
                    continuation.resume(returning: callbackURL.absoluteString)
                } else {
                    // A dismissed browser surfaces as `.canceledLogin`; normalise it so the screen
                    // can reset quietly without claiming sign-in failed (it didn't, they closed
                    // it). Escaping the wait arrives here too, by way of `cancel()`.
                    let cancelled = (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin
                    continuation.resume(
                        throwing: cancelled ? AllodiaSignInError.cancelled
                            : (error ?? AllodiaSignInError.cancelled))
                }
            }
            session.presentationContextProvider = self
            // NOT ephemeral: reuse the browser's existing session, so somebody already signed in to
            // their Allodia account is not asked for a password again.
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: AllodiaSignInError.couldNotStart)
            }
        }
    }

    /// Opens a page of the service's in the same in-app browser tab, and does not wait for a
    /// redirect because there is not going to be one.
    ///
    /// The same session type on purpose. An in-app browser tab **is** the system browser, so the
    /// cookie a sign-in just set is already there and the page opens signed in; a `WKWebView` has
    /// its own jar, would show a login form instead, and Google refuses an embedded user-agent for
    /// the sign-in that form offers. Closing it resolves as a cancellation, which is the ordinary
    /// ending here and is discarded.
    func present(url: String) {
        guard let parsed = URL(string: url) else { return }
        let session = ASWebAuthenticationSession(
            url: parsed,
            callbackURLScheme: AllodiaOAuthConfig.callbackScheme
        ) { @Sendable _, _ in }
        session.presentationContextProvider = self
        session.prefersEphemeralWebBrowserSession = false
        self.session = session
        _ = session.start()
    }

    func presentationAnchor(for _: ASWebAuthenticationSession) -> ASPresentationAnchor {
        #if os(macOS)
        return NSApp.keyWindow ?? NSApp.windows.first ?? ASPresentationAnchor()
        #else
        let keyWindow = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first { $0.isKeyWindow }
        return keyWindow ?? ASPresentationAnchor()
        #endif
    }
}
