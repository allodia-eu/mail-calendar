// The macOS half of the MCP `create_draft` tool: turning an assistant's draft into the app's own
// composer, prefilled and unsent (docs/mcp.md). The core hands the draft over the `AgentHostUi`
// port; this puts it on the model, and the shell opens the composer the user already knows.
//
// Deliberately NOT a send. The user sees the recipients and the body and presses Send themselves.
// That is a visibility property, not a safety guarantee, someone who asked for "reply to Bob"
// will press Send without reading, so it is not the control the design leans on (the
// known-recipient guard is). What it does buy is that an assistant's message appears in the
// user's own app, where it can be edited or abandoned.

import Foundation
import MailcalBindings
import Security

#if canImport(AppKit)
import AppKit
#endif

/// One draft an assistant asked to open, with an identity of its own.
///
/// The id is a fresh `UUID` per request rather than anything derived from the draft: asking twice
/// for the same message is two requests, and a value-derived id would make the second one look
/// like the first and silently do nothing.
struct AgentDraftRequest: Identifiable, Equatable {
    let id = UUID()
    let draft: AgentDraft

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

/// The host side of the core's agent-UI port.
///
/// `openComposer` is called from the MCP server's connection task, off the main thread, and the
/// port's contract says an implementation must not block, a host that waited for the window
/// would stall that connection. So this hops to the main actor and returns immediately.
///
/// The model reference is bound to that actor rather than left free, and that is what lets this be
/// `Sendable` while holding a `weak` (so necessarily mutable) stored property. The actor is the
/// synchronisation, so both sides of the hop are checked rather than asserted.
final class AgentComposerBridge: AgentHostUi {
    @MainActor private weak var model: MailboxModel?

    @MainActor init(model: MailboxModel) {
        self.model = model
    }

    func openComposer(draft: AgentDraft) {
        Task { @MainActor in
            self.model?.pendingAgentDraft = AgentDraftRequest(draft: draft)
            #if canImport(AppKit)
            // Bring the app forward: a draft the user cannot see is not the review step this
            // whole design is built around. `ignoringOtherApps` because the request came from
            // another process, the assistant is frontmost, and without it the window would
            // merely bounce in the Dock.
            NSApplication.shared.activate(ignoringOtherApps: true)
            #endif
        }
    }
}

/// Where the local MCP server listens on this Mac.
enum McpEndpoint {
    /// The socket path for a given data-directory name, or `nil` on a platform with no endpoint.
    ///
    /// Beside the store, so the dev and production namespaces (`DevNamespace`) get separate
    /// sockets for free and a debug build cannot serve a release build's clients.
    ///
    /// # Why the real home when unsandboxed, and the GROUP container when sandboxed
    ///
    /// Unsandboxed (the Developer-ID `.dmg` and the dev build), `~/.local/share/…` is 50 bytes and
    /// reachable by both sides, so it is simply the right answer.
    ///
    /// Under the App Sandbox it is not reachable at all: `~/.local/share/…` is redirected into
    /// `~/Library/Containers/eu.allodia.mailcal/Data/…`, which is **93 bytes before the username**
    /// over the 104-byte `sun_path` limit for a long user name, and is another app's container
    /// from the relay's point of view, gated by `kTCCServiceSystemPolicyAppData` on macOS 15+.
    ///
    /// The App **Group** container is the way through, and it is the mechanism Apple documents for
    /// a sandboxed app and its helper to share a resource. MEASURED (2026-08-03), a sandboxed
    /// relay bundle carrying `com.apple.security.application-groups` completes a full JSON-RPC
    /// round trip over a socket here; with the entitlement removed and nothing else changed, the
    /// same `connect()` fails `EPERM`. Two things that do NOT work and look like they should:
    /// `com.apple.security.temporary-exception.files.home-relative-path.read-write` over the real
    /// home (file exceptions do not cover a socket `connect()`), and
    /// `com.apple.security.network.client` (`AF_INET`/`AF_INET6` only, the same reason no
    /// `network.server` is needed to listen).
    ///
    /// The group path is **66 bytes before the user name**, so it holds up to a 37-byte user name.
    /// That is the tightest of the three (the real home is 37 before the name) and the reason the
    /// core's length check is load-bearing rather than decorative: over it, the server refuses to
    /// start with a message naming the limit instead of a `bind()` that fails `ENAMETOOLONG`.
    static func path(dataDirName: String) -> String? {
        #if os(macOS)
        socketPath(
            groupContainer: sandboxedGroupContainer(),
            home: FileManager.default.homeDirectoryForCurrentUser,
            dataDirName: dataDirName
        )
        #else
        // iOS and iPadOS are excluded by construction: the OS suspends the app, and a server that
        // is asleep when a client connects is worse than none. Returning nil here means no
        // #if is needed anywhere else, the core simply never gets an endpoint.
        nil
        #endif
    }

    /// The App Group whose container the sandboxed build shares with the relay.
    ///
    /// Must match `com.apple.security.application-groups` in BOTH `AllodiaMail.appstore.
    /// entitlements` and `AllodiaMailMcpHelper.appstore.entitlements`, the group is the entire
    /// grant, so a typo in either file is a relay that launches and then cannot connect.
    ///
    /// No team-id prefix. macOS takes a `group.`-style identifier (measured 2026-08-03), and it is
    /// 5 bytes shorter than the team-prefixed form, which matters, because `sun_path` is the
    /// binding constraint on this path and nothing else about it can be shortened.
    static let appGroupIdentifier = "group.\(Brand.appID)"

    /// Where the socket goes, given what this build can reach. Pure, so both branches are tested
    /// the sandboxed one cannot be exercised by a test bundle that is not itself sandboxed, and
    /// an untested branch here is a Store build that silently has no MCP.
    static func socketPath(groupContainer: URL?, home: URL, dataDirName: String) -> String {
        if let groupContainer {
            // No namespace segment: only the sandboxed Store build takes this branch, and it is
            // always the production namespace (a dev build is ad-hoc signed and unsandboxed). The
            // 8 bytes a segment would cost come straight out of the user-name budget above.
            return groupContainer.appendingPathComponent("mcp.sock").path
        }
        return home.appendingPathComponent(".local/share/\(dataDirName)/mcp.sock").path
    }

    /// This build's App Group container, or `nil` when the build is not sandboxed.
    ///
    /// Gated on the sandbox rather than called unconditionally: `containerURL(for…)` returns a URL
    /// and CREATES the directory even for an unsandboxed process, so calling it blind would move
    /// the Developer-ID build's socket into `~/Library/Group Containers/`, a path it can reach
    /// today but that macOS 15+ gates behind `kTCCServiceSystemPolicyAppData`, trading a working
    /// flow for a TCC prompt. We read our OWN entitlements to decide, the same way
    /// `KeychainStore` picks its keychain, so one binary does the right thing under each signing.
    #if os(macOS)
    private static func sandboxedGroupContainer() -> URL? {
        guard let task = SecTaskCreateFromSelf(nil),
            SecTaskCopyValueForEntitlement(task, "com.apple.security.app-sandbox" as CFString, nil)
                as? Bool == true
        else { return nil }
        return FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: appGroupIdentifier)
    }
    #endif

    /// The key the generated snippet files this server under.
    ///
    /// **Letters, numbers, hyphens and underscores only**, and identical to the `name` the
    /// protocol advertises, one identifier, not two.
    ///
    /// It is tempting to use the display name here, because Claude Desktop labels a locally
    /// configured server by its config key and `"Allodia Mail & Calendar"` renders beautifully
    /// there. It is also a trap: Claude Code accepts only `[A-Za-z0-9_-]` in a server name, and
    /// **skips a Claude Desktop server whose name contains a space when importing it**. Where the
    /// name is embedded in a tool identifier, every other character is rewritten to `_`. So a
    /// pretty key looks right in one client and quietly breaks the others.
    ///
    /// The display name is `serverInfo.title`'s job, the spec is explicit that `title` is "for
    /// UI and end-user contexts" and `name` is "for programmatic or logical use". A client that
    /// shows the config key instead is a client that has not adopted that yet, and the fix
    /// belongs there rather than in a config key that has to work everywhere.
    static let configurationKey = "allodia-mail-and-calendar"

    /// The MCP-client configuration snippet for `endpoint`, ready to paste.
    ///
    /// Built with `JSONEncoder`, never string concatenation: the relay lives at
    /// `/Applications/Allodia Mail.app/Contents/MacOS/allodia-mcp`, and hand-built JSON would
    /// emit that space unescaped inside a path, producing a config file that looks right and
    /// parses wrong.
    static func configurationSnippet(endpoint: String) -> String {
        let server = ServerEntry(
            command: relayPath(),
            args: ["--endpoint", endpoint]
        )
        let config = ClientConfig(mcpServers: [configurationKey: server])
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        guard let data = try? encoder.encode(config),
              let json = String(data: data, encoding: .utf8)
        else {
            return ""
        }
        return json
    }

    /// Where the relay lives inside this app bundle, relative to `Contents/`.
    ///
    /// It is a nested **`.app`**, not the bare Mach-O it used to be, and that is a hard
    /// requirement rather than tidiness: a bare executable carrying
    /// `com.apple.security.app-sandbox` has no `CFBundleIdentifier`, so the sandbox has no
    /// container to attach and the process dies in `_libsecinit_appsandbox` **before `main()`**:
    /// `SIGTRAP`, every time, with or without `com.apple.security.inherit` (MEASURED 2026-08-03;
    /// `inherit` additionally cannot apply here, since the relay's parent is the user's assistant
    /// and there is no sandbox to inherit). Wrapped in a bundle it launches, gets its container,
    /// and an MCP client spawns the inner executable by absolute path exactly as before.
    ///
    /// One layout for BOTH macOS flows. The Developer-ID build does not need the bundle, but two
    /// layouts would mean two relay paths, two signing rules and two things to keep true; the
    /// sandbox forces the shape, so both wear it.
    static let relayBundlePath = "Library/Helpers/allodia-mcp.app"

    /// The relay executable inside this app bundle, falling back to a bare name on `PATH` when it
    /// is not there (a `swift run` from a checkout, where there is no bundle to look in).
    private static func relayPath() -> String {
        #if canImport(AppKit)
        let nested = Bundle.main.bundleURL
            .appendingPathComponent("Contents/\(relayBundlePath)/Contents/MacOS/allodia-mcp")
        if FileManager.default.isExecutableFile(atPath: nested.path) {
            return nested.path
        }
        #endif
        return "allodia-mcp"
    }

    private struct ClientConfig: Encodable {
        let mcpServers: [String: ServerEntry]
    }

    private struct ServerEntry: Encodable {
        let command: String
        let args: [String]
    }
}
