// The read-only in-app viewer for the current diagnostic log: monospace, opening scrolled to the
// end, the newest entries are what a support session is after, with a jump-to-end button for after
// the user has scrolled up. View-only by design: the log is a diagnostic record, so there is
// nothing here to edit, only to read, select, and copy.
//
// Two drawings of the same log. macOS lays it out as ONE document (`SelectableLog`), because
// selection there has to cross lines: a reader copying a passage into a support request wants the
// lines around the one that failed. Everywhere else it is one lazy row per line, which keeps a
// ~1 MB file off the main thread at the cost of selection stopping at a row.

import MailcalBindings
import SwiftUI

/// Splits the raw log text into the viewer's rows. Every appended entry ends in `\n`, so a
/// naive split always yields one phantom empty row at the end, drop exactly that, keeping any
/// real blank line inside the log. A plain function so the rule is testable without a view.
func logLines(_ text: String) -> [String] {
    guard !text.isEmpty else { return [] }
    var lines = text.components(separatedBy: "\n")
    if lines.last == "" {
        lines.removeLast()
    }
    return lines
}

/// The viewer sheet: the current `mailcal.log`, newest last. Presented from the Diagnostics
/// settings surface on both macOS and iOS.
struct DiagnosticsLogViewer: View {
    let close: () -> Void

    @State private var text = ""
    @State private var lines: [String] = []
    @State private var loaded = false
    /// Bumped by the jump-to-end button. macOS scrolls on the change rather than on a value,
    /// so pressing it twice from the same place still works.
    @State private var jumpRequests = 0

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(L10n.diagnostics_log_heading()).font(.headline)
                Spacer()
                Button(L10n.action_done(), action: close).keyboardShortcut(.defaultAction)
            }
            .padding()
            Divider()
            if loaded, lines.isEmpty {
                Text(L10n.diagnostics_log_empty())
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                logBody
            }
        }
        #if os(macOS)
        // A macOS sheet sizes to fit, give the log room without swallowing the screen.
        .frame(width: 680, height: 480)
        #endif
        .task {
            // One synchronous read off the log's own queue (≤ 1 MB, milliseconds); splitting
            // happens once here so scrolling never re-parses.
            text = FileLog.shared.readCurrentLog()
            lines = logLines(text)
            loaded = true
        }
    }

    /// macOS reads the log as one document, every other platform as a lazy per-line list.
    @ViewBuilder private var logBody: some View {
        #if os(macOS)
        SelectableLog(text: text, jumpRequests: jumpRequests)
            .overlay(alignment: .bottomTrailing) {
                if !lines.isEmpty {
                    Button {
                        jumpRequests += 1
                    } label: {
                        Label(L10n.diagnostics_jump_to_end(), systemImage: "arrow.down.to.line")
                    }
                    .padding(12)
                }
            }
        #else
        lineList
        #endif
    }

    /// The lazy per-line list. `defaultScrollAnchor(.bottom)` opens it at the end (newest
    /// last); the overlay button jumps back there after the user scrolled up.
    ///
    /// Selection is per `Text` here, so a drag cannot cross rows. That is the cost of the
    /// laziness the file size needs, and it is why macOS draws the log as one document instead.
    private var lineList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(lines.indices, id: \.self) { index in
                        // A truly empty Text collapses to zero height and the blank line
                        // vanishes; a single space keeps the row.
                        Text(lines[index].isEmpty ? " " : lines[index])
                            .font(.system(.caption, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .id(index)
                    }
                }
                .padding(10)
                .textSelection(.enabled)
            }
            .defaultScrollAnchor(.bottom)
            .overlay(alignment: .bottomTrailing) {
                if !lines.isEmpty {
                    Button {
                        proxy.scrollTo(lines.count - 1, anchor: .bottom)
                    } label: {
                        Label(L10n.diagnostics_jump_to_end(), systemImage: "arrow.down.to.line")
                    }
                    .padding(12)
                }
            }
        }
    }
}

#if os(macOS)
/// The log as one selectable document, which is what lets a drag cross lines.
///
/// A `LazyVStack` of `Text` rows scrolls a megabyte cheaply, and SwiftUI scopes text selection to
/// one `Text`, so the reader could select a line but never the three lines that explain it. Copying
/// a passage out is most of why the log is on screen at all. `NSTextView` selects across the whole
/// document and pays for it in nothing the reader notices: TextKit lays out what is visible, not
/// what is loaded, which is how Console.app draws far more than this.
///
/// Read-only on purpose: the log is a record, so there is nothing here to edit, only to read,
/// select and copy. `isEditable = false` keeps the caret and the keyboard out while leaving
/// selection, Find and Services in.
private struct SelectableLog: NSViewRepresentable {
    let text: String
    /// Any change scrolls to the bottom. A counter rather than a flag, so the button works
    /// again from the place it just took you.
    let jumpRequests: Int

    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator {
        var appliedJump: Int?
    }

    func makeNSView(context _: Context) -> NSScrollView {
        let scroll = NSTextView.scrollableTextView()
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        guard let view = scroll.documentView as? NSTextView else { return scroll }
        view.isEditable = false
        view.isSelectable = true
        view.isRichText = false
        view.drawsBackground = false
        view.font = .monospacedSystemFont(ofSize: NSFont.smallSystemFontSize, weight: .regular)
        view.textContainerInset = NSSize(width: 8, height: 8)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let view = scroll.documentView as? NSTextView else { return }
        let arrived = view.string != text
        if arrived {
            view.string = text
        }
        // The log opens at its end, because the newest entries are what a support session is
        // after. Scrolling is deferred to the next turn of the run loop: right after `string` is
        // set the layout manager has not sized the document yet, so scrolling now lands part-way
        // up a text view that is still growing underneath it.
        let jumped = context.coordinator.appliedJump != jumpRequests
        context.coordinator.appliedJump = jumpRequests
        if arrived || jumped {
            Task { @MainActor in view.scrollToEndOfDocument(nil) }
        }
    }
}
#endif
