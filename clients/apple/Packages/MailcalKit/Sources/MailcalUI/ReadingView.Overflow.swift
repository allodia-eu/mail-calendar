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
    ///
    /// ⚠️ **A `Button` and a popover, not a `Menu`, and that is the point.** A SwiftUI `Menu`
    /// carries its own chrome: it takes its height from neither the button style, nor a frame
    /// around it, nor one on its label, so it drew shorter than everything beside it and no
    /// amount of sizing moved it. Built from the same pieces as `toolbarButton`, this one is the
    /// same height as its neighbours because it is the same control, at both of the row's widths
    /// and whatever control size the platform picks. It also gives `ViewThatFits` a finite ideal
    /// width to measure, which a `Menu` does not: unbounded, that measurement traps in
    /// `_FlexFrameLayout` and the app dies on the first message opened.
    ///
    /// `iconsOnly` is the row's, not this button's: it shows no title either way, but the row's
    /// buttons are as tall as their content, and the two rows carry different content.
    func overflowMenu(iconsOnly: Bool) -> some View {
        Button {
            overflowOpen = true
        } label: {
            if exporting {
                ProgressView().controlSize(.small)
            } else if iconsOnly {
                Image(systemName: "ellipsis").frame(minWidth: 24, minHeight: 24)
            } else {
                Label(L10n.a11y_more_actions(), systemImage: "ellipsis")
                    .labelStyle(UntitledIconLabelStyle())
            }
        }
        .buttonStyle(.bordered)
        .controlSize(compactActions ? .large : .small)
        .accessibilityLabel(L10n.a11y_more_actions())
        .popover(isPresented: $overflowOpen, arrowEdge: .bottom) {
            overflowItems
                // Without this an iPhone presents a popover as a sheet, which is the whole
                // screen for one line of menu.
                .presentationCompactAdaptation(.popover)
        }
    }

    /// What the overflow holds: actions on the message as a document
    /// (`docs/reading-actions.md`).
    @ViewBuilder
    private var overflowItems: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                overflowOpen = false
                export()
            } label: {
                Label(L10n.action_save_as_eml(), systemImage: "square.and.arrow.down.on.square")
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(exporting)
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 10)
        .fixedSize()
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

/// Draws a label's icon alone, while its title still sets the height.
///
/// `.iconOnly` takes the title out of the layout as well as out of the drawing, and a button
/// using it comes out shorter than its labelled neighbours: their height is their title's line
/// box, which is taller than the glyph beside it. This keeps that line box and gives it no width,
/// which is what makes an icon-only button in a labelled row the same height as the rest of that
/// row (`docs/reading-actions.md`).
private struct UntitledIconLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 0) {
            configuration.icon
            configuration.title
                .lineLimit(1)
                .frame(width: 0)
                .hidden()
                .accessibilityHidden(true)
        }
    }
}
