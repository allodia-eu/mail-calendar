// Turning what the share sheet handed the extension into a drop the app can open
// (docs/os-integration.md).
//
// The staging is the whole reason this exists. An `NSItemProvider` is a loan for the length of the
// request: the file it names may belong to another app, may be a temporary the system deletes the
// moment the completion handler returns, and is unreadable by the time the user presses Send. So
// the bytes are copied into the App Group container first, and only a path into that container is
// written down.
//
// Nothing here decides what a share means. The names and media types are passed on exactly as the
// sharing app gave them, because sanitising is `mailcal_composer::share`'s job, and doing it twice
// is how two answers appear for one file.

import Foundation
import MailcalShareBox
import UniformTypeIdentifiers

/// Reads a share request. On the main actor throughout, because `NSExtensionItem` and
/// `NSItemProvider` are not `Sendable` and may not leave the thread the request arrived on; the
/// copying itself is handed off (see `stage`), so nothing long-running happens here.
@MainActor
enum ShareIntake {

    /// Reads one share request and leaves it in `box`, or answers `nil` when it carried nothing.
    static func read(items: [NSExtensionItem], into box: ShareBox) async -> ShareDrop? {
        var drop = ShareDrop()
        // Swept now, while a share is the only thing touching this directory. A sweep at launch
        // instead would race the cold-start share that is staging during it.
        box.sweep()
        let staging = try? box.stagingDirectory(for: drop.id)
        var itemText = ""

        for item in items {
            if drop.subject.isEmpty, let title = item.attributedTitle?.string {
                drop.subject = title
            }
            if itemText.isEmpty, let text = item.attributedContentText?.string {
                itemText = text
            }
            for provider in item.attachments ?? [] {
                switch await take(provider, staging: staging) {
                case .file(let record): drop.files.append(record)
                case .text(let text) where drop.text.isEmpty: drop.text = text
                case .text, .nothing: break
                }
            }
        }
        // The item's own text is the fallback rather than the first answer: a browser puts the page
        // title there while the link itself arrives as an attachment, and the link is what belongs
        // in the message.
        if drop.text.isEmpty { drop.text = itemText }
        return drop.isEmpty ? nil : drop
    }

    /// What one attachment turned out to be.
    private enum Taken {
        case file(SharedFileRecord)
        case text(String)
        case nothing
    }

    /// Reads one attachment: a file to stage, or text to put in the body.
    ///
    /// The order of the questions is the whole rule. A file URL conforms to `public.url` as well,
    /// so asking about URLs first would turn every shared file into a line of text; and everything
    /// conforms to `public.item`, so that question can only be the last one.
    private static func take(_ provider: NSItemProvider, staging: URL?) async -> Taken {
        if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
            let source = await loadURL(provider, type: .fileURL)
            return .file(await stage(provider, from: source, in: staging))
        }
        if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
            // A web link is text as far as a message is concerned, and a `mailto:` link shared as
            // text is the one route a share has to the recipient fields: the core decodes it
            // through the same allowlist a tapped link goes through.
            return await loadURL(provider, type: .url).map { .text($0.absoluteString) } ?? .nothing
        }
        if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
            return await loadText(provider).map { .text($0) } ?? .nothing
        }
        if provider.hasItemConformingToTypeIdentifier(UTType.item.identifier) {
            // A photograph, a document from another app's storage, anything the provider will only
            // materialise on request. That copy happens inside the load, see `copyFileRepresentation`.
            let copied = await copyFileRepresentation(provider, into: staging)
            return .file(
                record(provider, path: copied?.path ?? "", name: copied?.lastPathComponent))
        }
        return .nothing
    }

    /// Copies a shared file into the staging directory and describes it.
    ///
    /// A file that could not be copied is recorded with **no path** rather than dropped. The core
    /// refuses a pathless item as `NoPath` and hands it back in `rejected`, so the user is told
    /// about a file they watched go into a share sheet rather than left to notice.
    ///
    /// The copy is detached: a share of several photographs is megabytes of I/O, and the thread it
    /// would otherwise run on is the one the sharing app is waiting on.
    private static func stage(
        _ provider: NSItemProvider, from source: URL?, in staging: URL?
    ) async -> SharedFileRecord {
        guard let source, let staging else {
            return record(provider, path: "", name: source?.lastPathComponent)
        }
        let destination = await Task.detached { copy(source, into: staging) }.value
        return record(provider, path: destination?.path ?? "", name: source.lastPathComponent)
    }

    private static func record(
        _ provider: NSItemProvider, path: String, name: String?
    ) -> SharedFileRecord {
        SharedFileRecord(
            path: path,
            suggestedName: provider.suggestedName ?? name ?? "",
            declaredMediaType: declaredMediaType(provider)
        )
    }

    /// What the sharing app said this is, as a media type.
    ///
    /// Offered to the core as a *declared* type, which it still validates and may overrule: a
    /// provider registers its types most specific first, and a generic tail such as `public.data`
    /// is exactly the case the core falls back to the extension for.
    private static func declaredMediaType(_ provider: NSItemProvider) -> String {
        provider.registeredTypeIdentifiers
            .lazy
            .compactMap { UTType($0)?.preferredMIMEType }
            .first ?? ""
    }

    // MARK: - The item provider, awaited

    private static func loadURL(_ provider: NSItemProvider, type: UTType) async -> URL? {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: type.identifier, options: nil) { value, _ in
                // Resolved to a Sendable value inside the handler: what the provider hands back is
                // `NSSecureCoding`, which may not cross the continuation.
                switch value {
                case let url as URL: continuation.resume(returning: url)
                case let data as Data:
                    continuation.resume(returning: URL(dataRepresentation: data, relativeTo: nil))
                case let text as String: continuation.resume(returning: URL(string: text))
                default: continuation.resume(returning: nil)
                }
            }
        }
    }

    private static func loadText(_ provider: NSItemProvider) async -> String? {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: UTType.plainText.identifier, options: nil) {
                value, _ in
                switch value {
                case let text as String: continuation.resume(returning: text)
                case let data as Data:
                    continuation.resume(returning: String(data: data, encoding: .utf8))
                default: continuation.resume(returning: nil)
                }
            }
        }
    }

    /// Asks the provider to materialise the file, and copies it **before the handler returns**.
    ///
    /// The URL a file representation hands over lives only for the length of that handler: the
    /// system deletes it as soon as it returns, so copying afterwards finds nothing there.
    ///
    /// Asked for by the provider's **own most specific type** rather than by `public.item`. Both
    /// resolve, since everything conforms to `public.item`, but the specific one is what makes a
    /// photograph arrive as the photograph rather than as whatever generic form the provider keeps
    /// for it.
    private static func copyFileRepresentation(
        _ provider: NSItemProvider, into staging: URL?
    ) async -> URL? {
        guard let staging else { return nil }
        let type = provider.registeredTypeIdentifiers.first ?? UTType.item.identifier
        return await withCheckedContinuation { continuation in
            _ = provider.loadFileRepresentation(forTypeIdentifier: type) { url, _ in
                continuation.resume(returning: url.flatMap { copy($0, into: staging) })
            }
        }
    }

    // MARK: - The copy itself

    /// Copies one file into `staging` under a name this filesystem accepts, or answers `nil` when
    /// it could not be read.
    ///
    /// `nonisolated`, and it has to be: this runs both from a detached task and from inside the
    /// provider's own completion handler, neither of which is the main actor.
    ///
    /// The name here is the staging directory's own, not the attachment's: the name a recipient
    /// reads comes back from the core, already normalised.
    nonisolated private static func copy(_ source: URL, into staging: URL) -> URL? {
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        let destination = unique(stagedName(source.lastPathComponent), in: staging)
        do {
            try FileManager.default.copyItem(at: source, to: destination)
            return destination
        } catch {
            // A provider that withdrew the loan, a file that vanished, a sender that lied about
            // what it was offering. One unreadable item must not cost the user the rest of the
            // share, and the caller records it as pathless so the app can still say so.
            return nil
        }
    }

    /// A name safe to create on this device's filesystem.
    nonisolated private static func stagedName(_ value: String) -> String {
        let forbidden = CharacterSet(charactersIn: "/\\:*?\"<>|").union(.controlCharacters)
        let cleaned = value.components(separatedBy: forbidden).joined(separator: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: ". _"))
            .prefix(80)
        return cleaned.isEmpty ? "shared" : String(cleaned)
    }

    /// The first free name, so sharing two files called `scan.pdf` stages two files.
    ///
    /// The number goes before the extension, not after it: the extension is what decides which
    /// application opens the file, and `scan.pdf-2` opens in none.
    nonisolated private static func unique(_ name: String, in directory: URL) -> URL {
        let candidate = directory.appendingPathComponent(name)
        guard FileManager.default.fileExists(atPath: candidate.path) else { return candidate }
        let stem = candidate.deletingPathExtension().lastPathComponent
        let suffix = candidate.pathExtension
        for attempt in 2...99 {
            var next = directory.appendingPathComponent("\(stem)-\(attempt)")
            if !suffix.isEmpty { next.appendPathExtension(suffix) }
            if !FileManager.default.fileExists(atPath: next.path) { return next }
        }
        return directory.appendingPathComponent("\(UUID().uuidString)-\(name)")
    }
}
