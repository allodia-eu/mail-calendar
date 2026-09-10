// The reading pane's overflow menu, at the end of the action row, and the .eml export behind
// it (docs/reading-actions.md). Split out of ReadingView.swift to keep it under 500 lines.

#if os(macOS)
import AppKit
#endif
import MailcalBindings
import SwiftUI

extension ReadingView {
    /// The menu at the end of the action row: the actions on the message as a document.
    ///
    /// An icon at every width, unlike the buttons beside it, because it is the one control whose
    /// place is what identifies it. Its accessible name is the same on every platform.
    var overflowMenu: some View {
        Menu {
            Button {
                export()
            } label: {
                Label(L10n.action_save_as_eml(), systemImage: "square.and.arrow.down.on.square")
            }
            .disabled(exporting)
        } label: {
            if exporting {
                ProgressView().controlSize(.small)
            } else {
                Image(systemName: "ellipsis").frame(minWidth: 24, minHeight: 24)
            }
        }
        .menuIndicator(.hidden)
        .buttonStyle(.bordered)
        .controlSize(compactActions ? .large : .small)
        .accessibilityLabel(L10n.a11y_more_actions())
        .frame(maxWidth: 44)
    }

    /// The file name to offer, from the subject this view *draws*, so an untitled message
    /// exports as "(no subject).eml" rather than under a second, English name.
    private var exportFileName: String {
        messageExportFileName(
            subject: message.subject.isEmpty ? L10n.mail_no_subject() : message.subject
        )
    }

    private func export() {
        guard !exporting else { return }
        exportError = nil
        #if os(macOS)
        let panel = NSSavePanel()
        panel.nameFieldStringValue = exportFileName
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            Task { @MainActor in
                // Spin only once a destination is chosen: the write, not the panel, is the wait.
                exporting = true
                defer { exporting = false }
                if !(await model.saveMessageSource(message.account, message.key, to: url)) {
                    exportError = L10n.message_save_failed()
                }
            }
        }
        #else
        // iPhone and iPad have no save panel: write to a temporary file, then present the share
        // sheet, which offers "Save to Files" and every other destination.
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("mailcal-exports", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent(exportFileName)
        Task { @MainActor in
            exporting = true
            defer { exporting = false }
            if await model.saveMessageSource(message.account, message.key, to: url) {
                PlatformShare.present(url)
            } else {
                exportError = L10n.message_save_failed()
            }
        }
        #endif
    }
}
