// A file an `NSItemProvider` holds, copied somewhere this process may keep it. The Share Extension
// does this with what a share sheet hands it, and the composer's photo picker with a chosen
// picture: both receive item providers, and a photograph reaches either one the same way.
//
// The copy is the point. The URL a provider hands over is a loan for the length of its completion
// handler: the system deletes the file as soon as the handler returns, so a path kept for later
// names nothing by the time the user presses Send.

import Foundation
import UniformTypeIdentifiers

public enum ProvidedFile {

    /// The type to ask a provider for its file by, from its registered types, most specific first.
    ///
    /// The first that is a file's bytes: a picture, a film, a document. A type that is not, such as
    /// a Live Photo bundle, which is a directory a message cannot carry, is passed over for one
    /// that is. Failing that, the first type this device does not know at all: an app's private
    /// type is still a file, and the sharing app can still deliver it. `nil` when there is neither.
    public static func fileType(_ identifiers: [String]) -> String? {
        identifiers.first { identifier in
            guard let type = UTType(identifier) else { return false }
            return type.conforms(to: .data) && !type.conforms(to: .directory)
        } ?? identifiers.first { UTType($0) == nil }
    }

    /// The name a copied file is staged under: the provider's own name when it gave one, with the
    /// extension of the bytes actually delivered.
    ///
    /// A provider's suggested name carries no extension for a photograph ("IMG_0042"), and the
    /// file it delivers is named after the provider's internals rather than the picture, so
    /// neither is the name alone. The extension is the delivered file's, because a photograph
    /// asked for in its compatible form arrives as a JPEG whatever it was taken as. A suggested
    /// name that ends in another type's extension (`IMG_0042.HEIC`) has it replaced; anything else
    /// after its last dot (`Report v1.2`) is part of the name.
    public static func name(suggested: String?, delivered: URL) -> String {
        let suffix = delivered.pathExtension
        guard let suggested, !suggested.isEmpty else { return delivered.lastPathComponent }
        guard let deliveredType = UTType(filenameExtension: suffix), !suffix.isEmpty
        else { return suggested }
        let own = (suggested as NSString).pathExtension
        guard let ownType = UTType(filenameExtension: own), !own.isEmpty, !ownType.isDynamic
        else { return "\(suggested).\(suffix)" }
        if ownType == deliveredType { return suggested }
        return "\((suggested as NSString).deletingPathExtension).\(suffix)"
    }

    /// Asks `provider` for its file and copies it into `directory` **before the handler returns**,
    /// or answers `nil` when it holds no file or the file could not be read.
    ///
    /// On the main actor because an `NSItemProvider` is not `Sendable` and both callers hold
    /// theirs there. The handler is not: the provider calls it on a queue of its own, and the copy
    /// runs there.
    @MainActor
    public static func copy(_ provider: NSItemProvider, into directory: URL) async -> URL? {
        guard let type = fileType(provider.registeredTypeIdentifiers) else { return nil }
        let suggested = provider.suggestedName
        return await withCheckedContinuation { continuation in
            _ = provider.loadFileRepresentation(forTypeIdentifier: type) { url, _ in
                continuation.resume(
                    returning: url.flatMap {
                        copy($0, into: directory, named: name(suggested: suggested, delivered: $0))
                    })
            }
        }
    }

    /// Copies one file into `directory` under a name this filesystem accepts, or answers `nil`
    /// when it could not be read.
    ///
    /// The name is the staged file's own, not the attachment's: the name a recipient reads comes
    /// back from the core, already normalised.
    public static func copy(_ source: URL, into directory: URL, named: String? = nil) -> URL? {
        let scoped = source.startAccessingSecurityScopedResource()
        defer { if scoped { source.stopAccessingSecurityScopedResource() } }
        // A folder is shareable from Finder, and it copies perfectly well: what it leaves behind
        // is a path the composer lists as an attachment and cannot read at Send. Refused here, so
        // it comes back as a named refusal instead; Linux's `readable_file` draws the same line.
        guard (try? source.resourceValues(forKeys: [.isRegularFileKey]))?.isRegularFile == true
        else { return nil }
        let destination = unique(stagedName(named ?? source.lastPathComponent), in: directory)
        do {
            try FileManager.default.createDirectory(
                at: directory, withIntermediateDirectories: true)
            try FileManager.default.copyItem(at: source, to: destination)
            return destination
        } catch {
            // A provider that withdrew the loan, a file that vanished, a sender that lied about
            // what it was offering. One unreadable item must not cost the user the rest.
            return nil
        }
    }

    /// A name safe to create on this device's filesystem, at most 80 characters.
    ///
    /// Shortened through its stem, so the extension survives: through the photo picker this is the
    /// name the attachment is sent under, and the extension is what decides which application opens
    /// it.
    static func stagedName(_ value: String) -> String {
        let limit = 80
        let forbidden = CharacterSet(charactersIn: "/\\:*?\"<>|").union(.controlCharacters)
        let cleaned = value.components(separatedBy: forbidden).joined(separator: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: ". _"))
        guard !cleaned.isEmpty else { return "shared" }
        guard cleaned.count > limit else { return cleaned }
        let suffix = (cleaned as NSString).pathExtension
        guard !suffix.isEmpty, suffix.count < limit / 2 else { return String(cleaned.prefix(limit)) }
        let stem = (cleaned as NSString).deletingPathExtension.prefix(limit - suffix.count - 1)
        return "\(stem).\(suffix)"
    }

    /// The first free name, so two files called `scan.pdf` stage as two files.
    ///
    /// The number goes before the extension, not after it: the extension is what decides which
    /// application opens the file, and `scan.pdf-2` opens in none.
    static func unique(_ name: String, in directory: URL) -> URL {
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
