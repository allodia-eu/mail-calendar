// Where the local MCP server listens on this Mac and the relay it names to the core
// (docs/mcp.md). Pure logic, no SwiftUI. The configuration snippet built from these is the core's,
// tested in `mailcal-mcp`.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

struct McpEndpointTests {
    @Test func theSandboxedBuildPutsTheSocketInTheGroupContainer() {
        // The branch a test bundle can NEVER take by running: this suite is not sandboxed, so
        // `path(dataDirName:)` always resolves to the real home here and the Store build's only
        // socket path would ship untested. Hence the pure function, the branch is chosen by an
        // argument rather than by the process's own entitlements.
        let group = URL(
            fileURLWithPath: "/Users/x/Library/Group Containers/group.\(Brand.appID)")
        let sandboxed = McpEndpoint.socketPath(
            groupContainer: group, home: URL(fileURLWithPath: "/Users/x"), dataDirName: "mailcal")
        #expect(sandboxed == "\(group.path)/mcp.sock")

        // Unsandboxed keeps the real-home path the Developer-ID build already ships. A change
        // that quietly moved THAT would break every existing user's pasted config.
        let direct = McpEndpoint.socketPath(
            groupContainer: nil, home: URL(fileURLWithPath: "/Users/x"), dataDirName: "mailcal")
        #expect(direct == "/Users/x/.local/share/mailcal/mcp.sock")
    }

    @Test func theGroupContainerPathHoldsForALongUserName() {
        // The group container is the TIGHTEST of the candidate paths (66 bytes before the user
        // name, against the real home's 37), so this pins the headroom rather than trusting that
        // "it fit on the machine where it was written". Over the limit the core refuses to start
        // the server, a visible error, not a mysterious ENAMETOOLONG, but a user name that no
        // longer fits is still a user with no MCP, so we want to know here. 37 bytes is the
        // budget; lengthening the group identifier spends it.
        let longName = String(repeating: "a", count: 37)
        let group = URL(
            fileURLWithPath:
                "/Users/\(longName)/Library/Group Containers/group.\(Brand.appID)")
        let path = McpEndpoint.socketPath(
            groupContainer: group, home: URL(fileURLWithPath: "/Users/\(longName)"),
            dataDirName: "mailcal")
        #expect(
            path.utf8.count < 104,
            "the group-container socket path is \(path.utf8.count) bytes for a 32-byte user name"
        )
    }

    @Test func theAppGroupMatchesTheEntitlementsTheRelayIsSignedWith() throws {
        // The group string is the ENTIRE grant that lets the sandboxed relay reach the socket, and
        // it is written in three places (this constant, the app's Store entitlements, the relay
        // bundle's). A typo in any one of them is a relay that launches and then cannot connect:
        // which surfaces to the user as the app not running, pointing at nothing.
        //
        // The entitlements name the id as $(PRODUCT_BUNDLE_IDENTIFIER) because it is injected
        // (docs/branding.md), so what is compared here is that all three *derive* it the same way:
        // this constant from the bundle id, both files from the setting the build resolves to that
        // same id. The app group takes no $(AppIdentifierPrefix) templating, unlike the keychain
        // group beside it, that difference is deliberate and measured.
        let group = McpEndpoint.appGroupIdentifier
        #expect(group == "group.\(Brand.appID)")
        let templated = "group.$(PRODUCT_BUNDLE_IDENTIFIER)"

        let appDir = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()  // MailcalUITests
            .deletingLastPathComponent()  // Tests
            .deletingLastPathComponent()  // MailcalKit
            .deletingLastPathComponent()  // Packages
            .deletingLastPathComponent()  // clients/apple
            .appendingPathComponent("App")
        for file in ["AllodiaMail.appstore.entitlements", "AllodiaMailMcpHelper.appstore.entitlements"] {
            let text = try String(contentsOf: appDir.appendingPathComponent(file), encoding: .utf8)
            #expect(
                text.contains("com.apple.security.application-groups"),
                "\(file) does not grant the app group, the relay cannot reach the socket"
            )
            #expect(
                text.contains("<string>\(templated)</string>"),
                "\(file) names a different app group than McpEndpoint.appGroupIdentifier (\(group))"
            )
        }
    }

    @Test func theRelayIsANestedBundleBecauseABareExecutableCannotBeSandboxed() {
        // MEASURED, not assumed (2026-08-03): a bare Mach-O signed with app-sandbox dies in
        // _libsecinit_appsandbox before main(), no CFBundleIdentifier, so no container. The
        // Store requires the entitlement (ITMS-90296), so the bundle is the only shape that both
        // uploads and runs. This pins the path the config snippet hands the user against the
        // layout package.sh signs and project.yml creates.
        #expect(McpEndpoint.relayBundlePath == "Library/Helpers/\(Brand.mcpRelayName).app")
    }

    @Test func theSocketSitsBesideTheStoreSoDevAndReleaseCannotCollide() throws {
        #if os(macOS)
        let dev = try #require(McpEndpoint.path(dataDirName: "mailcal-dev"))
        let release = try #require(McpEndpoint.path(dataDirName: "mailcal"))
        #expect(dev != release, "a debug build must not serve a release build's clients")
        #expect(dev.hasSuffix("/mailcal-dev/mcp.sock"))

        // The `sun_path` limit is 104 bytes INCLUDING the terminator, and the sandbox container
        // path alone is 93 before the username. The real-home path this returns has to stay well
        // inside it, or bind() fails with an ENAMETOOLONG nobody would trace to a path length.
        #expect(
            release.utf8.count < 104,
            "the shipped socket path is \(release.utf8.count) bytes, at or over the sun_path limit"
        )
        #else
        // No endpoint on iOS/iPadOS, the OS suspends the app, so a server that is asleep when a
        // client connects is worse than none. This is what excludes mobile by construction: the
        // core is simply never told where to listen.
        #expect(McpEndpoint.path(dataDirName: "mailcal") == nil)
        #endif
    }

    @Test func theHostNamesTheRelayBesideTheEndpoint() throws {
        #if os(macOS)
        let host = try #require(McpEndpoint.host(dataDirName: "mailcal-dev"))
        #expect(host.endpoint == McpEndpoint.path(dataDirName: "mailcal-dev"))
        #expect(host.relayCommand.hasSuffix(Brand.mcpRelayName))
        #expect(host.relayArgs.isEmpty, "the core adds the endpoint itself")
        #else
        #expect(McpEndpoint.host(dataDirName: "mailcal") == nil)
        #endif
    }
}
