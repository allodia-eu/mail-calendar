// The Diagnostics settings surface: the one shared view both the categorised macOS
// Settings window and the iOS settings sheet mount, so the two cannot drift. It surfaces the
// rotating diagnostic log (docs/logging.md) without a cable: what it holds and how big it is,
// an in-app viewer, a share flow that states the privacy note BEFORE the file leaves the
// device, a copy-path shortcut, and the "include more detail" (DEBUG) opt-in, persisted via
// DiagnosticsPrefs and applied live through `MailcalApp.setLogLevel`.

import MailcalBindings
import SwiftUI

struct DiagnosticsSettingsView: View {
    var model: MailboxModel

    @State private var snapshot = FileLog.shared.snapshot()
    @State private var showingViewer = false
    @State private var confirmingShare = false
    @State private var pathCopied = false
    @State private var debugLogging = DiagnosticsPrefs.debugLogging

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            group(L10n.diagnostics_log_heading(), L10n.diagnostics_log_description()) {
                statusRows
                Divider()
                Button(L10n.diagnostics_view_log()) { showingViewer = true }
                shareControl
                copyPathRow
            }
            group(L10n.diagnostics_debug_heading(), L10n.diagnostics_debug_description()) {
                Toggle(L10n.diagnostics_debug_heading(), isOn: debugBinding)
                    .labelsHidden()
            }
        }
        .onAppear { snapshot = FileLog.shared.snapshot() }
        .sheet(isPresented: $showingViewer) {
            DiagnosticsLogViewer { showingViewer = false }
        }
    }

    // MARK: Status, how big the store is, and the cap that bounds it

    @ViewBuilder
    private var statusRows: some View {
        LabeledContent(
            L10n.diagnostics_log_size_label(),
            value: ByteCountFormatter.string(
                fromByteCount: Int64(snapshot.totalBytes), countStyle: .file
            )
        )
        LabeledContent(L10n.diagnostics_log_backups_label(), value: "\(snapshot.backupCount)")
        Text(L10n.diagnostics_log_cap_note())
            .font(.caption)
            .foregroundStyle(.secondary)
    }

    // MARK: Share, the privacy note is visible BEFORE the file leaves the device

    /// macOS uses `ShareLink` (the native service picker), which offers no confirm step to hang
    /// the note on, so the note sits directly above the control instead, as the cross-platform
    /// contract allows. iOS confirms first: the dialog carries the note as its message, and
    /// only its confirm button hands the file to the share sheet.
    @ViewBuilder
    private var shareControl: some View {
        #if os(macOS)
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.diagnostics_share_privacy_note())
                .font(.caption)
                .foregroundStyle(.secondary)
            ShareLink(item: FileLog.shared.fileURL) {
                Text(L10n.diagnostics_share_log())
            }
        }
        .padding(.top, 4)
        #else
        Button(L10n.diagnostics_share_log()) { confirmingShare = true }
            // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, and a
            // popover DROPS the `.cancel`-role button, so this read as one destructive button with no
            // way out. See the remove-account alert in Mailcal.swift for the full note.
            .alert(
                L10n.diagnostics_share_confirm_title(),
                isPresented: $confirmingShare
            ) {
                Button(L10n.diagnostics_share_log()) {
                    PlatformShare.present(FileLog.shared.fileURL)
                }
                Button(L10n.action_cancel(), role: .cancel) {}
            } message: {
                Text(L10n.diagnostics_share_privacy_note())
            }
        #endif
    }

    // MARK: Copy path, for power users, with transient confirmation

    private var copyPathRow: some View {
        HStack(spacing: 8) {
            Button(L10n.diagnostics_copy_path()) { copyPath() }
            if pathCopied {
                Text(L10n.diagnostics_path_copied())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .transition(.opacity)
            }
        }
    }

    private func copyPath() {
        PlatformPasteboard.copy(snapshot.path)
        withAnimation { pathCopied = true }
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 2_000_000_000)
            withAnimation { pathCopied = false }
        }
    }

    // MARK: The DEBUG toggle, persisted, and applied to the live core at once

    private var debugBinding: Binding<Bool> {
        Binding(
            get: { debugLogging },
            set: {
                debugLogging = $0
                model.setDiagnosticsDebugLogging($0)
            }
        )
    }

    // MARK: The same labelled GroupBox section both host screens use

    @ViewBuilder
    private func group(
        _ heading: String,
        _ description: String,
        @ViewBuilder content: () -> some View
    ) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text(heading).font(.headline)
                Text(description).font(.callout).foregroundStyle(.secondary)
                content()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }
}
