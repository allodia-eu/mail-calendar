// "Sign in with your provider" for a JMAP account, the browser half of the flow, and the sibling
// of MicrosoftOAuth.swift. As there, the Rust core owns the OAuth state machine (PKCE, token
// exchange, refresh) and this host owns only the browser hop: opening the authorization URL and
// capturing the redirect via ASWebAuthenticationSession, which reuses the system browser's session
// so an already-signed-in user usually isn't asked to log in again.
//
// One thing differs from Microsoft/Google, and it is why there is no client id here: a JMAP server
// is not an integration. It may be Fastmail, or a Stalwart on someone's NAS, and we have no
// registration with either, so the core discovers the authorization server and registers this
// install itself (RFC 9728 → 8414 → 7591) before it builds the authorization URL. The only piece
// the host owns is the redirect URI below, plus the Keychain writer for a rotated refresh token.
//
// Discovery is allowed to fail, it means "this server doesn't do this", never a dead end. The
// setup form only shows the button when `jmapOauthAvailable` said yes, and the password/API-token
// field stays right there either way.

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import AuthenticationServices
import Foundation
import MailcalBindings

/// The one piece of OAuth configuration this client owns for JMAP: where the authorization server
/// sends the browser back to.
enum JmapOAuthConfig {
    /// A custom scheme on the app's own bundle identifier. Nothing is registered with a provider
    /// (the core registers this install per server, dynamically), and nothing is declared in the
    /// Info.plist: `ASWebAuthenticationSession` claims the scheme for the life of the session, which
    /// is exactly how the Microsoft (`msauth.eu.allodia.mailcal://auth`) and Google iOS flows
    /// already work here, neither appears in `project.yml` either.
    ///
    /// It rides through to the token endpoint verbatim on every later refresh, so changing it would
    /// break every account connected under the old value.
    static let redirectURI = "\(Brand.appID)://jmap-oauth"

    /// The scheme part of `redirectURI` (before `://`), which ASWebAuthenticationSession watches
    /// for to know the flow is done.
    static var callbackScheme: String {
        String(redirectURI.prefix { $0 != ":" })
    }
}

/// A failure in the browser half of the flow.
enum JmapSignInError: LocalizedError {
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

/// Drives one ASWebAuthenticationSession sign-in: opens the authorization URL in the system browser
/// and resolves with the redirect callback URL the browser returns to our custom scheme. The exact
/// shape of `MicrosoftSignIn`, the two flows differ only in which scheme they wait for.
///
/// The caller must hold this alive for the whole flow: the session's `presentationContextProvider`
/// is a **weak** reference, so a released provider leaves the browser with nowhere to present.
@MainActor
final class JmapSignIn: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    /// Opens `authorizationURL` and awaits the redirect, returning the full callback URL.
    /// Throws on user cancel or a session error.
    func authorize(authorizationURL: String) async throws -> String {
        guard let url = URL(string: authorizationURL) else {
            throw JmapSignInError.badURL
        }
        return try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: url,
                callbackURLScheme: JmapOAuthConfig.callbackScheme
            ) { callbackURL, error in
                if let callbackURL {
                    continuation.resume(returning: callbackURL.absoluteString)
                } else {
                    // A user dismissing the browser surfaces as `.canceledLogin`; normalise it
                    // to our `.cancelled` so the form can quietly reset without claiming sign-in
                    // failed (it didn't, they closed it).
                    let cancelled = (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin
                    continuation.resume(
                        throwing: cancelled ? JmapSignInError.cancelled
                            : (error ?? JmapSignInError.cancelled))
                }
            }
            session.presentationContextProvider = self
            // NOT ephemeral: we want the browser's existing session with the provider, so a
            // signed-in user skips re-entering their password (the whole UX win).
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: JmapSignInError.couldNotStart)
            }
        }
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
