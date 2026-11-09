// The Mac window's one leading edge: New Mail, the sidebar's heading and its chevron column all
// start where the close button does, so the window's left side reads as a single line from the
// traffic lights down. Read from AppKit rather than written down, because the buttons have moved
// between macOS releases and will again.

import SwiftUI
#if os(macOS)
import AppKit

/// Reports the x at which the window's close button starts, in window coordinates.
struct WindowButtonsLeading: NSViewRepresentable {
    @Binding var leading: CGFloat

    func makeNSView(context: Context) -> NSView {
        Probe { x in if leading != x { leading = x } }
    }

    func updateNSView(_ view: NSView, context: Context) {}

    private final class Probe: NSView {
        let report: (CGFloat) -> Void

        init(report: @escaping (CGFloat) -> Void) {
            self.report = report
            super.init(frame: .zero)
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) { nil }

        // It only measures, so a click goes to whatever is under it, the window included.
        override func hitTest(_ point: NSPoint) -> NSView? { nil }

        private var watched: NSObjectProtocol?

        // The buttons move after the window first appears: a window gains its toolbar after its
        // content, and the title bar grows and moves them with it. So the frame is followed, not
        // read once.
        // Leaving the window comes through here too, with no window, which is where the
        // observer is let go.
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            watched.map(NotificationCenter.default.removeObserver)
            watched = nil
            guard let close = window?.standardWindowButton(.closeButton) else { return }
            close.postsFrameChangedNotifications = true
            watched = NotificationCenter.default.addObserver(
                forName: NSView.frameDidChangeNotification, object: close, queue: .main
            ) { [weak self] _ in self?.measure(close) }
            measure(close)
        }

        private func measure(_ close: NSButton) {
            let x = close.convert(close.bounds, to: nil).minX
            // Out of the view update this may arrive in: SwiftUI state is not written mid-update.
            DispatchQueue.main.async { [report] in report(x) }
        }
    }
}

/// Where the sidebar outline puts a row's content and a heading's text, from the window's edge.
/// Measured: AppKit owns both and offers no way to read them.
private let outlineRowInset: CGFloat = 16
private let outlineHeaderInset: CGFloat = 14

extension ContentView {
    /// How far each sidebar row moves so its chevron column starts at the close button.
    var paneRowShift: CGFloat { max(0, chromeLeading - outlineRowInset) }

    /// How far the sidebar's heading moves so its text starts at the close button.
    var paneHeaderShift: CGFloat { max(0, chromeLeading - outlineHeaderInset) }
}
#else
extension ContentView {
    /// The iPad's column has no window buttons to line up on.
    var paneRowShift: CGFloat { 0 }
    var paneHeaderShift: CGFloat { 0 }
}
#endif
