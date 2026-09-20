// The drop box a share is handed over in (docs/os-integration.md).
//
// A Share Extension is a process of its own, so it cannot open the composer: the composer needs the
// accounts, the signatures and the send path, and those belong to the one core with the one open
// store (docs/reading-window.md). What the extension can do is copy the shared bytes somewhere the
// app can read them and leave a note saying what arrived. That note is a "drop", and this is the
// box both processes agree on.
//
// Where the box is depends on what the build was signed with, and `shared(appID:)` carries the
// measurement. Nothing here decides what a share MEANS: the names, the media types, the cap and
// the refusals belong to `mailcal_composer::share`, which the app asks once it has a drop.
//
// Deliberately free of MailcalBindings, and so of the Rust XCFramework: an iOS Share Extension is
// held to a far smaller memory budget than an app, and loading the core to copy a file would spend
// it on nothing.

import Foundation

/// One file a share carried, staged where both processes can read it.
///
/// `suggestedName` and `declaredMediaType` are passed on **exactly as the sharing app gave them**,
/// unsanitised: sanitising is the core's job, and doing it twice is how two answers appear for one
/// file.
public struct SharedFileRecord: Codable, Equatable, Sendable {
    /// Where the staged bytes are, inside the group container.
    public var path: String
    /// The display name the sharing app offered, or blank.
    public var suggestedName: String
    /// The media type the sharing app declared, or blank.
    public var declaredMediaType: String

    public init(path: String, suggestedName: String, declaredMediaType: String) {
        self.path = path
        self.suggestedName = suggestedName
        self.declaredMediaType = declaredMediaType
    }
}

/// One share, as the extension recorded it and the app finds it.
///
/// The three payload fields are `ShareRequest`'s, in the same order, so the app's whole job on
/// picking one up is to hand it to the core.
public struct ShareDrop: Codable, Equatable, Sendable {
    /// Identifies this share, and names the directory its files were staged in.
    public var id: String
    /// When the extension wrote it, which is the order the app opens drops in.
    public var receivedAt: Date
    /// The files, in the order the user selected them.
    public var files: [SharedFileRecord]
    /// Text the share carried: a selection, a URL, or a whole `mailto:` link. May be blank.
    public var text: String
    /// A subject the sharing app suggested (a browser shares the page title this way). May be
    /// blank.
    public var subject: String

    public init(
        id: String = UUID().uuidString,
        receivedAt: Date = Date(),
        files: [SharedFileRecord] = [],
        text: String = "",
        subject: String = ""
    ) {
        self.id = id
        self.receivedAt = receivedAt
        self.files = files
        self.text = text
        self.subject = subject
    }

    /// Whether this drop is worth waking a composer for.
    ///
    /// The core answers the same question properly, once it has normalised everything; this is the
    /// cheap one the extension asks before writing a note about a share that carried nothing.
    public var isEmpty: Bool {
        files.isEmpty && text.isEmpty && subject.isEmpty
    }
}

/// Where a share waits between the extension writing it and the app opening it.
///
/// Built over a directory rather than over the App Group name, so the rules below are exercised by
/// tests against a temporary directory instead of only inside a signed build.
public struct ShareBox: Sendable {
    /// The App Group container, or any directory standing in for one.
    public let container: URL

    public init(container: URL) {
        self.container = container
    }

    /// Where a build with no App Group keeps the box, relative to the user's home.
    ///
    /// Its own directory rather than a corner of the engine store's: it is the one place a Share
    /// Extension is granted, and the store, the credential index and the log are not things an
    /// extension should be able to open. It is also deliberately outside the per-dev-account
    /// namespaces (`DevNamespace`), like the diagnostic log, because a share belongs to whichever
    /// build the user is running, not to a store.
    public static let homeRelativePath = ".local/share/mailcal-share"

    /// The box this build shares with its Share Extension, or `nil` where it has neither way in.
    ///
    /// Two locations, and which one it is, is decided by what this build was signed with rather
    /// than by a flag:
    ///
    /// - **An App Group**, wherever a provisioning profile granted one: iOS, and the Mac App Store
    ///   build. It is the only place a sandboxed app and a sandboxed extension can both write.
    /// - **A directory under the user's home**, on macOS otherwise. ⚠️ MEASURED 2026-09-20: a
    ///   sandboxed process cannot use an App Group its profile did not grant, and a macOS build
    ///   outside the Store has no profile at all; `containerURL` still answers with a path, and
    ///   every write to it fails with EPERM. So the extension is given a home-relative sandbox
    ///   exception to this one directory instead, and the app, unsandboxed in that build, reads it
    ///   as an ordinary path.
    public static func shared(appID: String) -> ShareBox? {
        if let group = entitledGroup(appID: appID),
            let container = FileManager.default.containerURL(
                forSecurityApplicationGroupIdentifier: group) {
            return ShareBox(container: container)
        }
        #if os(macOS)
        return ShareBox(container: home().appendingPathComponent(homeRelativePath, isDirectory: true))
        #else
        return nil
        #endif
    }

    /// The App Group this build may actually use, or `nil` when it has none.
    ///
    /// The running signature is asked rather than trusted to be a constant, which is the same
    /// "read our own entitlements" gate `KeychainStore` and `McpEndpoint` each use: one binary's
    /// worth of source then behaves correctly under every signing, and a group that was left off
    /// because no profile could grant it reads as absent rather than as broken. iOS has no
    /// `SecTask` to ask, and needs none: every iOS build is provisioned.
    static func entitledGroup(appID: String) -> String? {
        let declared = "group.\(appID)"
        #if os(macOS)
        guard let task = SecTaskCreateFromSelf(nil),
            let groups = SecTaskCopyValueForEntitlement(
                task, "com.apple.security.application-groups" as CFString, nil) as? [String]
        else { return nil }
        return groups.first { $0.hasSuffix(declared) }
        #else
        return declared
        #endif
    }

    #if os(macOS)
    /// The user's real home, which under the App Sandbox is **not** what `FileManager` reports:
    /// there `homeDirectoryForCurrentUser` is the container, and the extension's grant is for the
    /// path outside it. `getpwuid` escapes the container, which is exactly what is wanted here and
    /// is a trap everywhere else.
    private static func home() -> URL {
        guard let entry = getpwuid(getuid()), let directory = entry.pointee.pw_dir else {
            return FileManager.default.homeDirectoryForCurrentUser
        }
        return URL(fileURLWithPath: String(cString: directory))
    }
    #endif

    /// Where the notes are, one file per share.
    private var inbox: URL { container.appendingPathComponent("share-inbox", isDirectory: true) }

    /// Where the bytes are, one directory per share, so finishing or sweeping one share never
    /// reaches another's files.
    private var staging: URL {
        container.appendingPathComponent("share-staging", isDirectory: true)
    }

    /// The directory this share's files are copied into, created if it is not there yet.
    public func stagingDirectory(for dropID: String) throws -> URL {
        let directory = staging.appendingPathComponent(dropID, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    /// Leaves a note that this share arrived.
    ///
    /// Written under a temporary name and then moved into place, because the app reads this
    /// directory on every activation and a half-written note would read as a corrupt share rather
    /// than as an unfinished one. The move is atomic within one filesystem, and both ends of it are
    /// in the same container.
    public func deposit(_ drop: ShareDrop) throws {
        try FileManager.default.createDirectory(at: inbox, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(drop)
        let pending = inbox.appendingPathComponent("\(drop.id).writing")
        try data.write(to: pending, options: .atomic)
        try FileManager.default.moveItem(
            at: pending, to: inbox.appendingPathComponent("\(drop.id).json"))
    }

    /// Takes the share that has waited longest, and leaves the rest.
    ///
    /// One at a time, because one composer opens at a time. A second share waiting behind this one
    /// keeps its place and its files, and is picked up on the next activation.
    public func take() -> ShareDrop? {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        for note in notes() {
            defer { try? FileManager.default.removeItem(at: note) }
            // A note we cannot read is a note we will never read, so it goes either way: leaving it
            // would make every activation from here on try it again and reach the real share behind
            // it never.
            if let data = try? Data(contentsOf: note),
                let drop = try? decoder.decode(ShareDrop.self, from: data) {
                return drop
            }
        }
        return nil
    }

    /// The notes waiting, oldest first.
    private func notes() -> [URL] {
        let listed = (try? FileManager.default.contentsOfDirectory(
            at: inbox,
            includingPropertiesForKeys: [.contentModificationDateKey],
            options: [.skipsHiddenFiles])) ?? []
        return listed
            .filter { $0.pathExtension == "json" }
            .sorted { modified($0) < modified($1) }
    }

    private func modified(_ url: URL) -> Date {
        (try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate)
            ?? .distantPast
    }

    /// How long a staged copy is kept before a later share clears it away.
    ///
    /// Generous, because the only thing it must outlive is a composer someone left open: the staged
    /// path is what Send reads, and sweeping underneath one would lose the attachment at the moment
    /// the user finally acted. A week is far longer than any real draft and still bounds the
    /// directory. The Windows client keeps the same week for the same reason.
    public static let retention: TimeInterval = 7 * 24 * 60 * 60

    /// Deletes staged files older than ``retention``.
    ///
    /// Called by the extension as it stages, which is the only moment this directory can have
    /// grown. A sweep at launch instead would race the cold-start share that is staging during it.
    public func sweep(now: Date = Date()) {
        let cutoff = now.addingTimeInterval(-Self.retention)
        let listed = (try? FileManager.default.contentsOfDirectory(
            at: staging,
            includingPropertiesForKeys: [.contentModificationDateKey],
            options: [.skipsHiddenFiles])) ?? []
        for directory in listed where modified(directory) < cutoff {
            try? FileManager.default.removeItem(at: directory)
        }
    }
}

/// How the extension asks for the app.
public enum ShareHandoff {
    /// The URI scheme the app registers so that opening it brings the app up.
    ///
    /// Its own scheme rather than the app id's, which sign-in redirects already use: a scheme is
    /// claimed app-wide, and claiming that one would route an OAuth redirect to the app's URL hook
    /// as well, where today every redirect is captured inside its own `ASWebAuthenticationSession`
    /// and none can arrive.
    public static func scheme(appID: String) -> String { "\(appID).share" }

    /// The URL the extension opens once it has left a drop.
    ///
    /// A doorbell and nothing more: it carries no payload, the app acts on being activated rather
    /// than on the URL, and a page that guesses the scheme can therefore do no more than make the
    /// app look in its own box. What was shared never travels in a URI
    /// (docs/os-integration.md).
    public static func doorbell(appID: String) -> URL? {
        URL(string: "\(scheme(appID: appID))://shared")
    }
}
