// The reading pane's attachment bar and the open/save actions behind it. Split out of
// ReadingView.swift to keep it under 500 lines.

#if os(macOS)
import AppKit
#endif
import MailcalBindings
import SwiftUI

extension ReadingView {
    @ViewBuilder
    var attachmentBar: some View {
        if let body = bodySnapshot, !body.attachments.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(L10n.attachments_title())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Spacer()
                    if let attachmentError {
                        Text(attachmentError)
                            .font(.caption)
                            .foregroundStyle(.red)
                    }
                }
                attachmentRows(body.attachments)
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 8)
        }
    }

    /// The attachment rows, hugging their content until they would crowd the message out, and
    /// scrolling from there.
    ///
    /// A message really can carry twenty files, and the bar sits **above** the body in a column
    /// that does not scroll, so left to grow it pushes the message itself off the bottom of the
    /// screen, with no way to reach either. The cap is what keeps the mail readable; the scroll is
    /// what keeps every attachment reachable.
    @ViewBuilder
    private func attachmentRows(_ attachments: [AttachmentRow]) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 6) {
                ForEach(attachments, id: \.id) { attachment in
                    attachmentRow(attachment)
                }
            }
            // Measured rather than capped by `maxHeight` alone: a `ScrollView` takes every point it
            // is offered, so two attachments would reserve the whole cap and leave a blank strip
            // over the message.
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { attachmentsHeight = $0 }
        }
        .frame(height: min(attachmentsHeight, attachmentBarCap))
        .scrollBounceBehavior(.basedOnSize)
    }

    @ViewBuilder
    private func attachmentRow(_ attachment: AttachmentRow) -> some View {
        HStack(spacing: 8) {
                        Image(systemName: "paperclip").foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(attachment.fileName)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Text("\(attachment.mediaType) · \(formatBytes(attachment.size))")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        // Open hands the file to the OS default handler (which runs the
                        // OS's own scan); we never render/execute an attachment in-app.
                        Button {
                            open(attachment)
                        } label: {
                            if openingIDs.contains(attachment.id) {
                                ProgressView().controlSize(.small)
                            } else {
                                Label(L10n.action_open(), systemImage: "arrow.up.forward.app")
                            }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .disabled(openingIDs.contains(attachment.id))
                        Button {
                            save(attachment)
                        } label: {
                            if savingIDs.contains(attachment.id) {
                                ProgressView().controlSize(.small)
                            } else {
                                Label(L10n.action_save(), systemImage: "square.and.arrow.down")
                            }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .disabled(savingIDs.contains(attachment.id))
                    }
        .contentShape(Rectangle())
        // Double-click is a shortcut for the Open button above.
        .onTapGesture(count: 2) {
            open(attachment)
        }
    }

    private func save(_ attachment: AttachmentRow) {
        guard !savingIDs.contains(attachment.id) else { return }
        attachmentError = nil
        #if os(macOS)
        let panel = NSSavePanel()
        panel.nameFieldStringValue = attachment.fileName.isEmpty ? "attachment" : attachment.fileName
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            // The core decodes + writes the whole part off the main actor (see the model), so a
            // large attachment doesn't beachball the app; the result comes back on the main actor.
            Task { @MainActor in
                // Spin only once a destination is chosen, the write, not the panel, is the wait.
                savingIDs.insert(attachment.id)
                defer { savingIDs.remove(attachment.id) }
                if !(await model.saveAttachment(message.account, message.key, attachment, to: url)) {
                    attachmentError = L10n.attachment_save_failed()
                }
            }
        }
        #else
        // iOS has no save panel: decode to a temp file, then present the share sheet (which
        // offers "Save to Files" and every other destination).
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("mailcal-attachments", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent(
            attachmentFileName(name: attachment.fileName, mediaType: attachment.mediaType)
        )
        Task { @MainActor in
            savingIDs.insert(attachment.id)
            defer { savingIDs.remove(attachment.id) }
            if await model.saveAttachment(message.account, message.key, attachment, to: url) {
                PlatformShare.present(url)
            } else {
                attachmentError = L10n.attachment_save_failed()
            }
        }
        #endif
    }

    private func open(_ attachment: AttachmentRow) {
        guard !openingIDs.contains(attachment.id) else { return }
        attachmentError = nil
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("mailcal-attachments", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let url = directory.appendingPathComponent(
            attachmentFileName(name: attachment.fileName, mediaType: attachment.mediaType)
        )
        do {
            try FileManager.default.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
        } catch {
            attachmentError = L10n.attachment_open_failed()
            return
        }
        openingIDs.insert(attachment.id)
        // Decode off the main actor (see the model), then hand the file to the OS: the default
        // handler on macOS, Quick Look, the system viewer, rendering in its own out-of-process
        // preview extension, on iOS/iPadOS. Either way the OS's own file scanning applies and we
        // never render or execute attachment content ourselves.
        Task { @MainActor in
            defer { openingIDs.remove(attachment.id) }
            let saved = await model.saveAttachment(message.account, message.key, attachment, to: url)
            guard saved else {
                attachmentError = L10n.attachment_open_failed()
                return
            }
            #if os(macOS)
            guard NSWorkspace.shared.open(url) else {
                attachmentError = L10n.attachment_open_failed()
                return
            }
            #else
            // Quick Look declines some types outright (an installer, an unknown binary); those
            // fall back to the share sheet, which offers the apps that can open them.
            if !PlatformQuickLook.present(url) {
                PlatformShare.present(url)
            }
            #endif
        }
    }

    private func formatBytes(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
    }
}
