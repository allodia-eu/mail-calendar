// The rich composer's files: the tiles the core's list is drawn as, between the header and the
// message, and the picker that adds to it (`docs/composer-layout.md`, rules 3 and 4).

#if os(macOS)
import AppKit
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
    let add: (String, [ComposerFileAttachment]) -> ComposerFiles
    let remove: (String, String) -> ComposerFiles
}

/// The narrowest a tile is drawn before the grid puts fewer in a row.
private let tileMinimumWidth: CGFloat = 180

extension RichComposeView {
    /// The files as tiles, in rows of equal-width columns that wrap and grow downward with no cap
    /// and no scroll of their own: the column around them is the one scroll.
    @ViewBuilder var fileTiles: some View {
        if !files.tiles.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.attachments_title())
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
            Image(systemName: "doc")
                .foregroundStyle(.secondary)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 1) {
                Text(tile.fileName)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if let size = tile.size {
                    Text(ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            .font(.caption)
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
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
        .accessibilityElement(children: .contain)
    }

    /// Hands files to the core and shows the list it answers with. Built from the list's binding
    /// and the verbs, never from `self`, because the editor holds it (`ComposerDropModifier`).
    var attachFiles: ([URL]) -> Void {
        let files = $files
        let verbs = fileVerbs
        let composition = composition
        return { urls in
            files.wrappedValue = verbs.add(composition, urls.map(PickedFile.describe))
        }
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
}
