// Persists the macOS split-view divider positions across launches, the macOS twin of the Windows
// PaneLayoutStore. SwiftUI's HSplitView doesn't surface NSSplitView's built-in autosave, so this
// zero-size helper, dropped into one pane via `.background(...)`, reaches up to the enclosing
// NSSplitView and sets its autosaveName. AppKit then saves and restores divider offsets to
// UserDefaults for free, matching this app's UserDefaults persistence convention (TimeZoneViews).

// Entirely macOS-only: NSSplitView autosave has no iOS analogue (iOS uses NavigationSplitView +
// @SceneStorage). The whole file compiles away on iOS; its one call site is guarded too.
#if os(macOS)
import AppKit
import SwiftUI

/// The **innermost** `NSSplitView` enclosing `view`, if there is one.
///
/// Innermost is the load-bearing word now that the macOS shell nests its splits (see
/// `ContentView.macOSLayout`): the probe dropped into the message list has both the mail split and
/// the sidebar split above it, and it must name the mail one. Returning the outer split from an
/// inner pane would point two different layouts at a single defaults key, so each destination
/// switch would overwrite the other's divider, the shape of the bug the nesting exists to fix.
func nearestSplitView(from view: NSView) -> NSSplitView? {
    var ancestor = view.superview
    while let candidate = ancestor {
        if let split = candidate as? NSSplitView { return split }
        ancestor = candidate.superview
    }
    return nil
}

/// A zero-size view that gives the SwiftUI `HSplitView` it sits inside a persistent divider, by
/// setting `autosaveName` on the backing `NSSplitView`.
struct SplitViewAutosave: NSViewRepresentable {
    let name: String

    func makeNSView(context: Context) -> NSView {
        let probe = NSView(frame: .zero)
        // The NSSplitView isn't an ancestor yet at make-time; defer the walk to the next runloop
        // turn, once SwiftUI has hosted the panes inside it.
        DispatchQueue.main.async { [weak probe] in
            guard let probe, let split = nearestSplitView(from: probe) else { return }
            if split.autosaveName != name { split.autosaveName = name }
        }
        return probe
    }

    func updateNSView(_ nsView: NSView, context: Context) {}
}
#endif
