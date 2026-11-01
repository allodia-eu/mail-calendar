// macOS Google sign-in binds a listener on loopback and hands Google the address it bound, because
// Google's Desktop client type redirects to `http://127.0.0.1:<port>/`. Two things are pinned here:
// the address the core is handed really is the port that got bound, and the bind says so in the
// diagnostic log. The second is what a support request has to fall back on, since the browser hop
// leaves nothing else behind: a build the App Sandbox denies `com.apple.security.network.server`
// never gets past this call, and every line before it reads like a healthy sign-in.
//
// The sandbox itself is out of reach from here (the suite runs unsandboxed, as do the Developer ID
// and dev builds), which is exactly why `cargo xtask check-store-sandbox` holds that half.

#if os(macOS)
import Foundation
import MailcalBindings
import Network
import Testing

@testable import MailcalUI

@MainActor
struct GoogleLoopbackTests {
    @Test func theRedirectAddressIsThePortThatWasBound() async throws {
        let flow = GoogleLoopbackFlow()
        let redirect = try await flow.redirectURI()

        let port = try #require(URL(string: redirect)?.port)
        #expect(port > 0)
        #expect(redirect == "http://127.0.0.1:\(port)/")
    }

    @Test func theBoundPortReachesTheDiagnosticLog() async throws {
        let flow = GoogleLoopbackFlow()
        let redirect = try await flow.redirectURI()
        let port = try #require(URL(string: redirect)?.port)

        // Rotation is checked before a write, so the line just appended is in the current file
        // whether or not the store rolled on the way in.
        #expect(
            FileLog.shared.readCurrentLog()
                .contains("google sign-in: waiting for the redirect on 127.0.0.1:\(port)"))
    }

    /// A browser can ask the port for something else before it delivers the redirect, and can
    /// write a request in pieces. Neither may end the wait: the listener answers the stray request
    /// and takes the redirect whole.
    @Test(.timeLimit(.minutes(1))) func onlyTheRedirectEndsTheWait() async throws {
        let flow = GoogleLoopbackFlow()
        let redirect = try await flow.redirectURI()
        let port = try #require(URL(string: redirect)?.port)

        // The wait is in place before the first request: both run on the main actor, in order.
        let wait = Task { try await flow.awaitRedirect {} }
        let favicon = try await URLSession.shared.data(
            from: try #require(URL(string: "http://127.0.0.1:\(port)/favicon.ico")))
        #expect((favicon.1 as? HTTPURLResponse)?.statusCode == 400)
        try await Self.send(
            ["GET /?state=s&co", "de=c HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"], to: port)
        let callback = try await wait.value
        #expect(callback == "http://127.0.0.1:\(port)/?state=s&code=c")
    }

    /// A redirect that comes back after the sign-in stopped waiting reads a page saying so, not a
    /// refused connection.
    @Test(.timeLimit(.minutes(1))) func aRedirectAfterTheWaitEndedReadsThatItEnded() async throws {
        let flow = GoogleLoopbackFlow()
        let redirect = try await flow.redirectURI()
        let port = try #require(URL(string: redirect)?.port)
        let wait = Task { try await flow.awaitRedirect {} }
        flow.cancel()
        _ = try? await wait.value

        let (body, response) = try await URLSession.shared.data(
            from: try #require(URL(string: "http://127.0.0.1:\(port)/?state=s&code=c")))
        #expect((response as? HTTPURLResponse)?.statusCode == 200)
        #expect(String(decoding: body, as: UTF8.self) == GoogleLoopbackFlow.page(L10n.sign_in_page_ended()))
    }

    /// Writes `parts` to the port one at a time over one connection, as a browser that sends a
    /// request in pieces does.
    nonisolated private static func send(_ parts: [String], to port: Int) async throws {
        let connection = NWConnection(
            host: "127.0.0.1", port: NWEndpoint.Port(rawValue: UInt16(port))!, using: .tcp)
        connection.start(queue: .global())
        for part in parts {
            try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, Error>) in
                connection.send(content: Data(part.utf8), completion: .contentProcessed { error in
                    if let error { done.resume(throwing: error) } else { done.resume() }
                })
            }
            try await Task.sleep(for: .milliseconds(50))
        }
    }
}
#endif
