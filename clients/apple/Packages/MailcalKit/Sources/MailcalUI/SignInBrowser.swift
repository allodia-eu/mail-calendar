// The browser this host opens for a sign-in the core holds (docs/waiting.md,
// docs/provider-oauth.md rule 13): the platform's auth session, which reuses the person's own
// browser sign-ins, or on the Mac a Google sign-in's loopback port in the default browser. It
// reports once how the browser ended, unless the core no longer holds the attempt and this host
// closed it.

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import AuthenticationServices
import Foundation
import MailcalBindings

@MainActor
final class SignInBrowser: NSObject, ASWebAuthenticationPresentationContextProviding {
    /// The core's attempt this browser is for.
    let attempt: UInt64
    /// It was opened. Each attempt opens one browser, whatever becomes of it.
    private(set) var opened = false
    private var session: ASWebAuthenticationSession?
    private var ended: (@MainActor (SignInBrowserEnd) -> Void)?
    /// It reported how it ended, or was closed: nothing more is said.
    private var done = false
    #if os(macOS)
    /// The loopback bound for a Google sign-in before it was asked for, and its address.
    private let loopback: (redirect: String, flow: GoogleLoopbackFlow)?

    init(attempt: UInt64, loopback: (redirect: String, flow: GoogleLoopbackFlow)? = nil) {
        self.attempt = attempt
        self.loopback = loopback
    }
    #else
    init(attempt: UInt64) {
        self.attempt = attempt
    }
    #endif

    /// Opens `url`, waiting for a return to `redirect`; `ended` hears how the browser ended.
    func open(
        url: String, redirect: String?, ended: @escaping @MainActor (SignInBrowserEnd) -> Void
    ) {
        opened = true
        self.ended = ended
        #if os(macOS)
        let bound = loopback?.redirect
        #else
        let bound: String? = nil
        #endif
        guard let parsed = URL(string: url),
              let listener = AccountSignInRules.listener(redirect: redirect, loopback: bound)
        else {
            logBrowserSignIn("account", "nothing here can wait for this sign-in's browser")
            finish(.notOpened)
            return
        }
        switch listener {
        case let .session(scheme): present(parsed, scheme: scheme, returns: true)
        // Nothing returns to the app's own scheme: only the person closes the session.
        case .server: present(parsed, scheme: Brand.appID, returns: false)
        case .loopback: openLoopback(url)
        }
    }

    /// The core no longer waits on this browser: it closes, saying nothing.
    func close() {
        guard !done else { return }
        done = true
        logBrowserSignIn("account", "the sign-in no longer waits on the browser; closing it")
        session?.cancel()
        session = nil
        #if os(macOS)
        loopback?.flow.cancel()
        #endif
    }

    private func finish(_ end: SignInBrowserEnd) {
        guard !done else { return }
        done = true
        session = nil
        ended?(end)
    }

    private func present(_ url: URL, scheme: String, returns: Bool) {
        let session = ASWebAuthenticationSession(
            url: url, callbackURLScheme: scheme
        ) { @Sendable callbackURL, error in
            let closedByPerson = (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin
            let callback = callbackURL?.absoluteString
            Task { @MainActor in
                self.sessionEnded(
                    callback: callback, closedByPerson: closedByPerson, returns: returns)
            }
        }
        session.presentationContextProvider = self
        // Not ephemeral: the browser's own sign-in with the provider is what spares the password.
        session.prefersEphemeralWebBrowserSession = false
        self.session = session
        logBrowserSignIn("account", "opening the browser")
        guard session.start() else {
            logBrowserSignIn("account", "the browser did not open")
            finish(.notOpened)
            return
        }
    }

    private func sessionEnded(callback: String?, closedByPerson: Bool, returns: Bool) {
        // A session this host closed still reports its end; it is not the person's.
        guard !done else { return }
        if let callback, returns {
            logBrowserSignIn("account", "the browser came back with the redirect")
            finish(.redirected(callbackUrl: callback))
        } else if closedByPerson || callback != nil {
            logBrowserSignIn("account", "the browser was closed")
            finish(.closed)
        } else {
            logBrowserSignIn("account", "the browser could not be shown")
            finish(.notOpened)
        }
    }

    private func openLoopback(_ url: String) {
        #if os(macOS)
        guard let flow = loopback?.flow else {
            finish(.notOpened)
            return
        }
        Task { [weak self] in
            do {
                let callback = try await flow.authorize(authorizationURL: url)
                self?.finish(.redirected(callbackUrl: callback))
            } catch GoogleSignInError.cancelled {
                self?.finish(.closed)
            } catch {
                self?.finish(.notOpened)
            }
        }
        #else
        finish(.notOpened)
        #endif
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
