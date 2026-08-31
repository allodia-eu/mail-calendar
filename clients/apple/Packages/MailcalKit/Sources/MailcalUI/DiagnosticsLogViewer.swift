// The read-only in-app viewer for the current diagnostic log: monospace, one lazy
// row per line (the file can be ~1 MB / thousands of lines, so eager Text would hang the sheet),
// opening scrolled to the end, the newest entries are what a support session is after, with a
// jump-to-end button for after the user has scrolled up. View-only by design: the log is a
// diagnostic record, so there is nothing here to edit, only to read, select, and copy.

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

    @State private var lines: [String] = []
    @State private var loaded = false

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
            lines = logLines(FileLog.shared.readCurrentLog())
            loaded = true
        }
    }

    /// The lazy per-line list. `defaultScrollAnchor(.bottom)` opens it at the end (newest
    /// last); the overlay button jumps back there after the user scrolls up.
    private var logBody: some View {
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
