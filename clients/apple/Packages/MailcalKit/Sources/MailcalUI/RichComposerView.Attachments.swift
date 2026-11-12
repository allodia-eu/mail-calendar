// The rich composer's attachment picker: the picked-file model, the footer the row list sits in,
// and the file-choose action. Split out of RichComposerView.swift to keep it under 500 lines.

#if os(macOS)
import AppKit
#endif
import Foundation
import MailcalBindings
import SwiftUI
import UniformTypeIdentifiers

struct PickedAttachment: Identifiable {
    let id = UUID()
    let url: URL
    /// The name and media type to send under, when the caller already knows them. A file the
    /// user picked does not: it is described by the file itself. A file the core staged does,
    /// because its path is a temporary name nobody would recognise, and what the recipient must
    /// see is the name the original's sender gave it.
    private let supplied: (fileName: String, mediaType: String)?

    init(url: URL) {
        self.url = url
        supplied = nil
    }

    /// One file the core staged: a share's, or one a forwarded message carries.
    init(staged: ComposerFileAttachment) {
        url = URL(fileURLWithPath: staged.path)
        supplied = (staged.fileName, staged.mediaType)
    }

    var fileName: String {
        if let supplied { return supplied.fileName }
        return url.lastPathComponent.isEmpty ? "attachment" : url.lastPathComponent
    }

    var mediaType: String {
        if let supplied { return supplied.mediaType }
        if let type = try? url.resourceValues(forKeys: [.contentTypeKey]).contentType,
           let mime = type.preferredMIMEType {
            return mime
        }
        if let type = UTType(filenameExtension: url.pathExtension),
           let mime = type.preferredMIMEType {
            return mime
        }
        return "application/octet-stream"
    }

    var composerFile: ComposerFileAttachment {
        ComposerFileAttachment(path: url.path, fileName: fileName, mediaType: mediaType)
    }
}

/// The most room the composer's file list takes before it scrolls: four rows, so a fifth shows the
/// list goes on.
let composerAttachmentCap: CGFloat = 96

extension RichComposeView {
    /// The attachment list and the prepare-failure note, everything below the editor.
    @ViewBuilder var composerFooter: some View {
        attachmentList
        if let composerError {
            Text(composerError)
                .font(.caption)
                .foregroundStyle(.red)
        }
        if let draftHint {
            Text(draftHint)
                .font(.caption)
                .foregroundStyle(draftHintFailed ? AnyShapeStyle(.red) : AnyShapeStyle(.secondary))
        }
    }

    var attachmentList: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !attachments.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L10n.attachments_title())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    attachmentRows
                }
                .padding(8)
                .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
            }
        }
    }

    /// One row per file, hugging them up to `composerAttachmentCap` and scrolling from there, as
    /// the reading pane's attachment bar does: left to grow, the list takes the editor's room and
    /// then runs off the bottom of the composer, and a file out of view cannot be checked or
    /// removed before sending. A file just added is scrolled to, so picking one shows it.
    private var attachmentRows: some View {
        ScrollViewReader { reader in
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(attachments) { attachment in
                        attachmentRow(attachment).id(attachment.id)
                    }
                }
                // Measured rather than capped by `maxHeight` alone: a `ScrollView` takes every
                // point it is offered, so one file would reserve the whole cap.
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                    attachmentRowsHeight = $0
                }
            }
            .frame(height: min(attachmentRowsHeight, composerAttachmentCap))
            .scrollBounceBehavior(.basedOnSize)
            .onChange(of: attachments.count) { before, after in
                guard after > before, let newest = attachments.last else { return }
                reader.scrollTo(newest.id, anchor: .bottom)
            }
        }
    }

    private func attachmentRow(_ attachment: PickedAttachment) -> some View {
        HStack(spacing: 6) {
            Image(systemName: "paperclip").foregroundStyle(.secondary)
            Text(attachment.fileName)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
            Button {
                attachments.removeAll { $0.id == attachment.id }
            } label: {
                Image(systemName: "xmark.circle")
            }
            .buttonStyle(.borderless)
            .help(L10n.action_remove())
        }
        .font(.caption)
    }

    func chooseAttachments() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.begin { response in
            guard response == .OK else { return }
            attachments.append(contentsOf: panel.urls.map { PickedAttachment(url: $0) })
        }
        #else
        PlatformFilePicker.present { urls in
            attachments.append(contentsOf: urls.map { PickedAttachment(url: $0) })
        }
        #endif
    }
}
