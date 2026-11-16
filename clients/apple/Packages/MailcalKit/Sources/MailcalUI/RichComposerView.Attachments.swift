// The rich composer's files: the tiles the core's list is drawn as, between the header and the
// message, and the picker that adds to it (`docs/composer-layout.md`, rules 3 and 4).

#if os(macOS)
import AppKit
#else
import PhotosUI
#endif
import Foundation
import MailcalBindings
import SwiftUI
import UniformTypeIdentifiers

/// How a file on this device is handed to the core: its path, its name, and its media type as the
/// system knows it. The core settles the name and the type it sends under.
enum PickedFile {
    static func describe(_ url: URL) -> ComposerFileAttachment {
        ComposerFileAttachment(
            path: url.path,
            fileName: url.lastPathComponent,
            mediaType: mediaType(url)
        )
    }

    private static func mediaType(_ url: URL) -> String {
        if let type = try? url.resourceValues(forKeys: [.contentTypeKey]).contentType,
           let mime = type.preferredMIMEType {
            return mime
        }
        return UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? ""
    }
}

/// The core verbs a composer's files go through. Closures over the model, so a preview or a test
/// can stand in for it.
struct ComposerFileVerbs {
    let read: (String) -> ComposerFiles
    /// Starts an addition, which runs Attach until `finish` ends it.
    let begin: (String) -> ComposerFiles
    /// Asynchronous: the core reads each file it is given, which is not a wait for the main thread.
    let finish: (String, [ComposerFileAttachment]) async -> ComposerFiles
    let remove: (String, String) -> ComposerFiles
}

/// The narrowest a tile is drawn before the grid puts fewer in a row: room for the square, a
/// name's start and end, its size and Remove on one line.
private let tileMinimumWidth: CGFloat = 220
/// The side of the square a tile starts with.
private let tileThumbnailSide: CGFloat = 28

extension RichComposeView {
    /// The files as tiles, in rows of equal-width columns that wrap and grow downward with no cap
    /// and no scroll of their own: the column around them is the one scroll.
    @ViewBuilder var fileTiles: some View {
        if !files.tiles.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.compose_files_summary(
                    count: files.tiles.count,
                    size: ByteCountFormatter.string(
                        fromByteCount: Int64(clamping: files.totalSize), countStyle: .file
                    )
                ))
                .font(.caption)
                .foregroundStyle(.secondary)
                LazyVGrid(
                    columns: [GridItem(.adaptive(minimum: tileMinimumWidth), spacing: 8)],
                    alignment: .leading,
                    spacing: 8
                ) {
                    ForEach(files.tiles, id: \.id) { tile in fileTile(tile) }
                }
            }
        }
    }

    private func fileTile(_ tile: ComposerFileTile) -> some View {
        HStack(spacing: 8) {
            ComposerFileThumbnail(preview: tile.preview, side: tileThumbnailSide)
            // Cut in the middle, so the part that tells one file from the next and the extension
            // both stay; the size keeps its own width.
            Text(tile.fileName)
                .lineLimit(1)
                .truncationMode(.middle)
                .font(.caption)
            if let size = tile.size {
                Text(ByteCountFormatter.string(fromByteCount: Int64(clamping: size), countStyle: .file))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .fixedSize()
            }
            Spacer(minLength: 0)
            ControlButton(
                title: L10n.action_remove(),
                systemImage: "xmark.circle",
                control: tile.remove
            ) {
                files = fileVerbs.remove(composition, tile.id)
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.borderless)
            .help(L10n.action_remove())
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 4)
        .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
        .accessibilityElement(children: .contain)
    }

    /// Starts adding files the person chose, so Attach runs while they are still being copied or
    /// read, and answers with what ends it: called once, with the files that arrived, none when
    /// nothing did (`docs/waiting.md`). Built from the list's binding and the verbs, never from
    /// `self`, because the editor holds it (`ComposerDropModifier`).
    var startAttaching: () -> ([URL]) -> Void {
        let files = $files
        let verbs = fileVerbs
        let composition = composition
        return {
            files.wrappedValue = verbs.begin(composition)
            return { urls in
                let picked = urls.map(PickedFile.describe)
                Task { @MainActor in
                    files.wrappedValue = await verbs.finish(composition, picked)
                }
            }
        }
    }

    /// Hands files that are on this device already to the core and shows the list it answers
    /// with, Attach running while the core reads them.
    var attachFiles: ([URL]) -> Void {
        let start = startAttaching
        return { urls in start()(urls) }
    }

    func chooseAttachments() {
        let attach = attachFiles
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.begin { response in
            guard response == .OK else { return }
            attach(panel.urls)
        }
        #else
        PlatformFilePicker.present { urls in attach(urls) }
        #endif
    }

    #if os(iOS)
    /// Attaches pictures and films from the photo library. Attach runs from the moment the picker
    /// closes, because each item is copied after it has, and a film is converted to its compatible
    /// form first. One that could not be copied (a picture the device could not download, say) is
    /// named in the composer's notice line rather than left out without a word.
    func choosePhotos() {
        let start = startAttaching
        let error = $composerError
        var finish: (([URL]) -> Void)?
        PlatformPhotoPicker.present(
            filter: .any(of: [.images, .videos]),
            limit: 0,
            onCopying: { finish = start() }
        ) { pick in
            finish?(pick.files)
            if let notice = pick.failureNotice { error.wrappedValue = notice }
        }
    }
    #endif
}

#if os(iOS)
extension PhotoPick {
    /// What the composer says when a chosen item could not be copied, or `nil` when all were.
    ///
    /// The share sheet's wording, because it is the same event: files the user chose that did not
    /// arrive.
    var failureNotice: String? {
        guard !failed.isEmpty else { return nil }
        let named = failed.filter { !$0.isEmpty }
        guard named.count == failed.count else { return L10n.compose_image_failed() }
        return L10n.compose_share_attachments_failed(
            files: ListFormatter.localizedString(byJoining: named))
    }
}
#endif
