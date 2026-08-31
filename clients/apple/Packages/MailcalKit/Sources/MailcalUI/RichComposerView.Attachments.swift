// The rich composer's attachment picker: the picked-file model, the row list, and the file-choose
// action. Split out of RichComposerView.swift to keep it under 500 lines.

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

    var fileName: String { url.lastPathComponent.isEmpty ? "attachment" : url.lastPathComponent }

    var mediaType: String {
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

extension RichComposeView {
    var attachmentList: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !attachments.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L10n.attachments_title())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(attachments) { attachment in
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
                }
                .padding(8)
                .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
            }
        }
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
