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
}
#endif
