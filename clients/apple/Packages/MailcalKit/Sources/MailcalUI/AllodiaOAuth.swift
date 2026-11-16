// The **Allodia account**'s browser: where its sign-in returns to, and the in-app browser tab its
// own pages open in. The sign-in itself is the core's one browser sign-in, which the browser
// driver opens and hands back as it does an account's (MailcalModel.BrowserSignIn.swift).
//
// An Allodia account is not a mail account: it carries no mailbox, appears in no switcher, and a
// token issued for it cannot touch anyone's mail. Its screen is Settings → Allodia account, never
// the setup wizard.
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

    /// The scheme part of `redirectURI` (before `://`).
    static var callbackScheme: String {
        String(redirectURI.prefix { $0 != ":" })
    }
}

/// Opens one of the account service's own pages, Manage account or the page that ends its
/// session, and waits for nothing.
///
/// The caller must hold this alive while the page is up: the session's
/// `presentationContextProvider` is a **weak** reference, so a released provider leaves the
/// browser with nowhere to present.
@MainActor
final class AllodiaPage: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    /// Opens `url` in an in-app browser tab, which **is** the system browser, so the cookie a
    /// sign-in set is already there and the page opens signed in; a `WKWebView` has its own jar,
    /// would show a login form instead, and Google refuses an embedded user-agent for the sign-in
    /// that form offers. Closing it resolves as a cancellation, which is discarded.
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
