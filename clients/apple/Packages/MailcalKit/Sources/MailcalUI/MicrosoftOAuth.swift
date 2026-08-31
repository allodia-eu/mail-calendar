// Microsoft 365 (OAuth) sign-in for the shared Apple client. The Rust core owns the OAuth state
// machine (PKCE, token exchange, refresh) and the token lifecycle; this host owns only the
// browser half, opening the authorization URL and capturing the redirect, via
// ASWebAuthenticationSession, which reuses the system default browser's login session so a
// user already signed in to Microsoft in their browser usually isn't asked to log in again.
// The captured callback URL goes straight back to the core (`completeMicrosoftLogin`).

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import MailcalBindings
import AuthenticationServices
import Foundation

/// The half of the Azure app registration that stays with the host. The client id is injected
/// into the core at build time; `redirectURI` cannot be, because Azure registers it against this
/// app's bundle identifier, register it **exactly** under the app's "Mobile and desktop
/// applications" platform in the Azure portal. There is deliberately no client secret anywhere on
/// the device: PKCE, owned by the core, stands in for one.
enum MicrosoftOAuthConfig {
    /// The tenant to authenticate against: `common` (work + personal accounts),
    /// `organizations`, `consumers`, or a specific tenant id.
    static let tenant = "common"
    /// The redirect URI, must match a redirect registered under "Mobile and desktop
    /// applications" in the Azure portal, character for character.
    static let redirectURI = "msauth.\(Brand.appID)://auth"

    /// The scheme part of `redirectURI` (before `://`), which ASWebAuthenticationSession
    /// watches for to know the flow is done.
    static var callbackScheme: String {
        String(redirectURI.prefix { $0 != ":" })
    }
}

/// A failure in the browser half of the flow.
enum MicrosoftSignInError: LocalizedError {
    case badURL
    case cancelled
    case couldNotStart

    var errorDescription: String? {
        switch self {
        case .badURL: return "The Microsoft sign-in URL was invalid."
        case .cancelled: return "Microsoft sign-in was cancelled."
        case .couldNotStart: return "Could not start Microsoft sign-in."
        }
    }
}

/// Drives one ASWebAuthenticationSession sign-in: opens the authorization URL in the
/// system browser and resolves with the redirect callback URL the browser returns to our
/// custom scheme. Retained by the model for the flow's duration.
@MainActor
final class MicrosoftSignIn: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    /// Opens `authorizationURL` and awaits the redirect, returning the full callback URL.
    /// Throws on user cancel or a session error.
    func authorize(authorizationURL: String) async throws -> String {
        guard let url = URL(string: authorizationURL) else {
            throw MicrosoftSignInError.badURL
        }
        return try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: url,
                callbackURLScheme: MicrosoftOAuthConfig.callbackScheme
            ) { callbackURL, error in
                if let callbackURL {
                    continuation.resume(returning: callbackURL.absoluteString)
                } else {
                    // A user dismissing the browser surfaces as `.canceledLogin`; normalise it
                    // to our `.cancelled` so the model can quietly reset without showing an error.
                    let cancelled = (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin
                    continuation.resume(
                        throwing: cancelled ? MicrosoftSignInError.cancelled
                            : (error ?? MicrosoftSignInError.cancelled))
                }
            }
            session.presentationContextProvider = self
            // NOT ephemeral: we want the browser's existing Microsoft session, so a
            // signed-in user skips re-entering their password (the whole UX win).
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: MicrosoftSignInError.couldNotStart)
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

/// The Keychain, as the core sees it: the *only* way an account's credential is written or erased
/// on this device. The core calls it when an account is added, when a refresh token rotates, when
/// a grant is re-authorised, and when an account is removed. Called from a Rust runtime thread;
/// `KeychainStore` is thread-safe.
///
/// One class for every provider family. There were three, with identical bodies, behind three
/// identical ports, which is what made forgetting one cheap.
///
/// Both methods throw rather than swallowing: the core decides what a refused write means, and it
/// decides differently depending on what it was doing (a failed add is rolled back; a failed
/// rotation cannot be). Reporting success on a write that did not happen is what the old,
/// return-less port made unavoidable.
final class KeychainCredentialStore: AccountCredentialStore {
    func persist(accountId: String, configToml: String) throws {
        guard KeychainStore.save(id: accountId, config: configToml) else {
            throw CredentialStoreError.Store("the Keychain refused to store this account")
        }
    }

    func delete(accountId: String) throws {
        guard KeychainStore.remove(id: accountId) else {
            throw CredentialStoreError.Store("the Keychain refused to erase this account")
        }
    }
}
