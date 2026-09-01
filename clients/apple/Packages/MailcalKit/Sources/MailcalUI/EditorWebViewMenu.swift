// The editor's right-click menu on macOS.
//
// WebKit's default menu carries items the composer must not offer: opening a link (navigation is
// blocked), downloading one, reloading the document, and Web Inspector. So the menu is filtered
// down to the editing actions, item by item, rather than replaced: each survivor keeps the
// platform's own label and keyboard equivalent, so it is already in the user's language.
//
// **The link item is the one that has to be here.** A link inside a quoted original cannot be
// clicked open in the composer, so without a way to copy its address it is text the user can see
// and not use (docs/composer-security.md, Gate 14).
//
// iOS/iPadOS need none of this: an editable web view already offers Cut/Copy/Paste in the system
// edit menu, and a long press on a link offers copying it.

#if os(macOS)
import AppKit
import WebKit

/// A `WKWebView` that shows only the editing actions in its context menu.
final class EditorWebView: WKWebView {
    /// WebKit's own identifiers for the items to keep. Matching on the identifier rather than the
    /// title is what makes this survive a localised menu.
    private static let allowed: Set<String> = [
        "WKMenuItemIdentifierCut",
        "WKMenuItemIdentifierCopy",
        "WKMenuItemIdentifierPaste",
        "WKMenuItemIdentifierCopyLink",
    ]

    override func willOpenMenu(_ menu: NSMenu, with event: NSEvent) {
        for item in menu.items where !Self.allowed.contains(item.identifier?.rawValue ?? "") {
            menu.removeItem(item)
        }
        // A menu emptied to nothing still opens, as an empty grey box; better to open none.
        while menu.items.first?.isSeparatorItem == true {
            menu.removeItem(at: 0)
        }
        while menu.items.last?.isSeparatorItem == true {
            menu.removeItem(at: menu.items.count - 1)
        }
    }
}
#endif
