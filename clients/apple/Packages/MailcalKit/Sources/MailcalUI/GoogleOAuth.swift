// Google (Gmail + Google Calendar) OAuth sign-in for the shared Apple client, the sibling of
// MicrosoftOAuth.swift. The Rust core owns the OAuth state machine (PKCE, token exchange,
// refresh), the client registration (injected at build time) and the token lifecycle; this host
// owns only the browser half, opening the authorization URL and capturing the redirect. The two
// Apple platforms use **different Google client types**, per Google's guidance: iOS/iPadOS an
// **iOS** client whose reversed-client-id custom scheme is captured by
// `ASWebAuthenticationSession` (`GoogleSignIn`); macOS a **Desktop** client whose
// `http://127.0.0.1:<port>/` loopback redirect is captured by a one-shot `NWListener`
// (`GoogleLoopbackFlow`, the sibling of Windows' `GoogleLoopback`), because Google deprecated the
// loopback redirect for the iOS client type and a desktop app cannot use the iOS custom-scheme
// flow. Both hand the captured callback URL straight back to the core (`completeGoogleLogin`).
// Google grants Gmail + Calendar scopes together at connect, so there is no separate
// calendar-reauth case (unlike Microsoft's reconnect-for-calendar).

#if os(macOS)
import AppKit
import Network
#else
import AuthenticationServices
import UIKit
#endif
import Foundation
import MailcalBindings

/// A failure in the browser half of the flow.
enum GoogleSignInError: LocalizedError {
    case badURL
    case cancelled
    case couldNotStart

    var errorDescription: String? {
        switch self {
        case .badURL: return "The Google sign-in URL was invalid."
        case .cancelled: return "Google sign-in was cancelled."
        case .couldNotStart: return "Could not start Google sign-in."
        }
    }
}

/// The host's browser half of a Google sign-in, abstracted over the two platform flows so the model
/// drives both identically: ask for the redirect URI, hand it to the core's `begin_google_login`,
/// open the returned authorization URL, and await the callback URL. iOS/iPadOS implement this with
/// `ASWebAuthenticationSession` (`GoogleSignIn`); macOS with a loopback listener
/// (`GoogleLoopbackFlow`).
@MainActor
protocol GoogleBrowserFlow: AnyObject {
    /// The redirect URI to pass to `begin_google_login`. iOS returns the static custom scheme;
    /// macOS starts its loopback listener and returns `http://127.0.0.1:<port>/`.
    func redirectURI() async throws -> String

    /// Opens `authorizationURL` in the browser and returns the full callback URL it is redirected
    /// to (carrying the `code` + `state` query). Throws `GoogleSignInError.cancelled` on user
    /// dismissal.
    func authorize(authorizationURL: String) async throws -> String
}

/// The platform's browser-half implementation: `ASWebAuthenticationSession` on iOS/iPadOS, a
/// loopback listener on macOS.
@MainActor
func makeGoogleBrowserFlow() -> GoogleBrowserFlow {
    #if os(macOS)
    GoogleLoopbackFlow()
    #else
    GoogleSignIn()
    #endif
}

#if !os(macOS)
/// iOS/iPadOS: drives one `ASWebAuthenticationSession` sign-in, opens the authorization URL in the
/// system browser and resolves with the redirect callback URL the browser returns to our custom
/// scheme. Retained by the model for the flow's duration.
@MainActor
final class GoogleSignIn: NSObject, GoogleBrowserFlow, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    /// The redirect this build's Google **iOS** client is registered for: the client id with its
    /// dotted components reversed, plus an arbitrary-but-required path, derived by the core from
    /// the injected client id so the two cannot drift. `nil` when the build carries no Google
    /// registration, the setup wizard then never offers the route, so reaching here would be a
    /// wiring bug rather than something a user can do.
    private static var registeredRedirectURI: String? { oauthRoutes().googleRedirectUri }

    /// The scheme part of the redirect (everything before `:/`), which
    /// ASWebAuthenticationSession watches for to know the flow is done.
    private static var callbackScheme: String? {
        registeredRedirectURI.map { String($0.prefix { $0 != ":" }) }
    }

    /// The iOS custom-scheme redirect is fixed config, no async work needed.
    func redirectURI() async throws -> String {
        guard let uri = Self.registeredRedirectURI else { throw GoogleSignInError.couldNotStart }
        return uri
    }

    /// Opens `authorizationURL` and awaits the redirect, returning the full callback URL.
    /// Throws on user cancel or a session error.
    func authorize(authorizationURL: String) async throws -> String {
        guard let url = URL(string: authorizationURL) else {
            throw GoogleSignInError.badURL
        }
        guard let scheme = Self.callbackScheme else {
            throw GoogleSignInError.couldNotStart
        }
        return try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: url,
                callbackURLScheme: scheme
            ) { callbackURL, error in
                if let callbackURL {
                    continuation.resume(returning: callbackURL.absoluteString)
                } else {
                    // A user dismissing the browser surfaces as `.canceledLogin`; normalise it
                    // to our `.cancelled` so the model can quietly reset without showing an error.
                    let cancelled = (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin
                    continuation.resume(
                        throwing: cancelled ? GoogleSignInError.cancelled
                            : (error ?? GoogleSignInError.cancelled))
                }
            }
            session.presentationContextProvider = self
            // NOT ephemeral: we want the browser's existing Google session, so a signed-in user
            // skips re-entering their password (the whole UX win).
            session.prefersEphemeralWebBrowserSession = false
            self.session = session
            if !session.start() {
                continuation.resume(throwing: GoogleSignInError.couldNotStart)
            }
        }
    }

    func presentationAnchor(for _: ASWebAuthenticationSession) -> ASPresentationAnchor {
        let keyWindow = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first { $0.isKeyWindow }
        return keyWindow ?? ASPresentationAnchor()
    }
}
#endif

#if os(macOS)
/// macOS: a one-shot loopback redirect listener, the Swift sibling of Windows' `GoogleLoopback`.
/// `redirectURI()` binds an `NWListener` to an ephemeral port on **127.0.0.1 only** (never a
/// routable interface, so the listener is unreachable off-box) and returns
/// `http://127.0.0.1:<port>/`; `authorize` opens the authorization URL in the user's default
/// browser (reusing their signed-in Google session) and awaits the single inbound redirect GET,
/// answering it with a small "you can close this tab" page. Google's Desktop client accepts any
/// loopback port, so nothing about the port is registered in the console. A public client, no
/// secret; PKCE (owned by the core) protects the exchange. Retained by the model for the flow's
/// duration; an abandoned flow (browser closed without finishing) leaves the listener waiting until
/// the model releases it, the same dangling-tab limitation as the Windows loopback flow.
@MainActor
final class GoogleLoopbackFlow: GoogleBrowserFlow {
    private var listener: NWListener?
    private var port: UInt16 = 0
    private var readyContinuation: CheckedContinuation<String, Error>?
    private var callbackContinuation: CheckedContinuation<String, Error>?
    private var didFinish = false

    /// Binds the loopback listener and returns the redirect URI once it is ready.
    func redirectURI() async throws -> String {
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        // Bind to loopback only (not every interface), matching the Windows 127.0.0.1 HttpListener
        // posture; port `.any` lets the OS pick a free ephemeral port, read back once `.ready`.
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        let listener = try NWListener(using: parameters)
        self.listener = listener
        listener.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in self?.handle(state: state) }
        }
        listener.newConnectionHandler = { [weak self] connection in
            Task { @MainActor in self?.handle(connection: connection) }
        }
        return try await withCheckedThrowingContinuation { continuation in
            self.readyContinuation = continuation
            listener.start(queue: .main)
        }
    }

    /// Opens the authorization URL in the default browser and awaits the loopback redirect.
    func authorize(authorizationURL: String) async throws -> String {
        guard let url = URL(string: authorizationURL) else { throw GoogleSignInError.badURL }
        return try await withCheckedThrowingContinuation { continuation in
            self.callbackContinuation = continuation
            // The user's default browser, reuses their existing Google session (the UX win) and is
            // where Google redirects to our loopback address on success.
            if !NSWorkspace.shared.open(url) {
                fail(GoogleSignInError.couldNotStart)
            }
        }
    }

    private func handle(state: NWListener.State) {
        switch state {
        case .ready:
            guard let assigned = listener?.port?.rawValue else { return }
            port = assigned
            readyContinuation?.resume(returning: "http://127.0.0.1:\(assigned)/")
            readyContinuation = nil
        case let .failed(error):
            fail(error)
        default:
            break
        }
    }

    private func handle(connection: NWConnection) {
        connection.start(queue: .main)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 16 * 1024) { [weak self] data, _, _, error in
            Task { @MainActor in self?.handle(connection: connection, data: data, error: error) }
        }
    }

    private func handle(connection: NWConnection, data: Data?, error: NWError?) {
        guard !didFinish else {
            connection.cancel()
            return
        }
        guard let data, let request = String(data: data, encoding: .utf8),
              let target = Self.requestTarget(request)
        else {
            if let error { fail(error) }
            connection.cancel()
            return
        }
        // Reconstruct the full callback URL the core validates (state) and exchanges (code):
        // http://127.0.0.1:<port>/?code=...&state=...&scope=...
        let callbackURL = "http://127.0.0.1:\(port)\(target)"
        writeClosePage(to: connection)
        finish(callbackURL: callbackURL)
    }

    /// Extracts the request target ("/?code=...") from an HTTP request's start line
    /// ("GET /?code=... HTTP/1.1"). Returns nil if the start line is malformed.
    static func requestTarget(_ request: String) -> String? {
        guard let startLine = request.split(whereSeparator: \.isNewline).first else { return nil }
        let fields = startLine.split(separator: " ")
        guard fields.count >= 2 else { return nil }
        return String(fields[1])
    }

    // The throwaway page the browser tab lands on after the redirect. Neutral, on-brand English;
    // there is no l10n key for this transient page (the same known gap as the Windows flow).
    // Self-contained (no external assets), so it renders offline.
    private func writeClosePage(to connection: NWConnection) {
        let html = """
        <!doctype html><html lang="en"><head><meta charset="utf-8">\
        <title>Allodia Mail &amp; Calendar</title></head>\
        <body style="font-family: system-ui, sans-serif; text-align: center; padding: 3rem;">\
        <p>You can close this tab and return to Allodia Mail &amp; Calendar.</p>\
        </body></html>
        """
        let response = "HTTP/1.1 200 OK\r\n"
            + "Content-Type: text/html; charset=utf-8\r\n"
            + "Content-Length: \(html.utf8.count)\r\n"
            + "Connection: close\r\n\r\n"
            + html
        connection.send(content: Data(response.utf8), completion: .contentProcessed { _ in
            connection.cancel()
        })
    }

    private func finish(callbackURL: String) {
        guard !didFinish else { return }
        didFinish = true
        callbackContinuation?.resume(returning: callbackURL)
        callbackContinuation = nil
        listener?.cancel()
        listener = nil
    }

    private func fail(_ error: Error) {
        guard !didFinish else { return }
        didFinish = true
        readyContinuation?.resume(throwing: error)
        readyContinuation = nil
        callbackContinuation?.resume(throwing: error)
        callbackContinuation = nil
        listener?.cancel()
        listener = nil
    }
}
#endif
