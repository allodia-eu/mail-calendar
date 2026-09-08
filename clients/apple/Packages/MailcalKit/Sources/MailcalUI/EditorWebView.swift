// What the composer's editor web view refuses on macOS: a context menu built for browsing, and a
// dropped file.
//
// The drop half is a dragging destination of this view's own, because the composer's SwiftUI one
// cannot cover this rectangle: `dropDestination` hit-tests the SwiftUI view tree, and an
// `NSViewRepresentable` is a hole in it. Nothing is registered anywhere up the chain, so there is
// no ancestor for a drag over the editor to fall through to; the composer took a file dropped on
// its From/To/Subject chrome and ignored one dropped on the message itself, which is the half a
// file is actually aimed at. What arrives here is handed straight to `ComposerDropModifier`, so
// both routes run the same code.
//
// WebKit's own dragged types are unregistered first, which keeps the drop away from the page: web
// code sees a `File` with no path, so it could neither stream it from disk on send nor list it for
// removal (docs/composer-security.md, Gate 13).
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
// iOS/iPadOS have no menu to filter: an editable web view already offers Cut/Copy/Paste in the
// system edit menu, and WebKit answers a long press on a link there by placing the caret, so no
// link menu is ever built. That leaves a link's address uncopyable in the composer on those hosts
// (docs/composer-security.md, Gate 14 "Known gaps").

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

    /// Separators carry no identifier, so the one filter takes them out with everything else: a
    /// right-click that lands on nothing keeps an empty menu, which AppKit does not open, rather
    /// than a box of dividers.
    ///
    /// ⚠️ **This reaches WebKit's items, not AppKit's.** Everything WebKit builds is in `menu`
    /// now, which is the whole browsing set. **AutoFill** is not in it at any point: AppKit draws
    /// that one itself while the menu is displayed, and it is in no `items` array to remove, so
    /// filtering here cannot take it off. See docs/composer-security.md, Gate 14 "Known gaps".
    override func willOpenMenu(_ menu: NSMenu, with event: NSEvent) {
        filter(menu)
    }

    /// Files dropped on the message itself, handed to the host.
    ///
    /// Set by `ComposerDropModifier`, which owns what a dropped file becomes; this type only gets
    /// the drag as far as it.
    var acceptDroppedFiles: (([URL]) -> Void)?

    /// Takes the drop over the editor for the host.
    ///
    /// ⚠️ **The SwiftUI `dropDestination` on the composer cannot reach here.** It hit-tests the
    /// SwiftUI view tree, and the editor is an `NSViewRepresentable`, so the rectangle it occupies
    /// is a hole in that tree: a file dropped on the From/To/Subject chrome was taken and the same
    /// file dropped on the message was ignored, which is the half a file is actually aimed at.
    /// Registering here is the Apple form of the capture-phase drop target Linux installs, for the
    /// same reason and with the same effect.
    ///
    /// The types are unregistered first because WebKit registers its own while building the view,
    /// and it would otherwise handle the drop internally, where the page can see only a `File` with
    /// no path (docs/composer-security.md, Gate 13). `viewDidMoveToWindow` rather than the
    /// initialiser: WebKit registers during construction and again on re-parenting, and this runs
    /// after both.
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        unregisterDraggedTypes()
        registerForDraggedTypes([.fileURL])
    }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        Self.fileURLs(from: sender).isEmpty ? [] : .copy
    }

    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        Self.fileURLs(from: sender).isEmpty ? [] : .copy
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        let urls = Self.fileURLs(from: sender)
        guard !urls.isEmpty else {
            return false
        }
        acceptDroppedFiles?(urls)
        return true
    }

    private static func fileURLs(from sender: NSDraggingInfo) -> [URL] {
        let options: [NSPasteboard.ReadingOptionKey: Any] = [.urlReadingFileURLsOnly: true]
        let urls = sender.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: options)
        return (urls as? [URL] ?? []).filter(\.isFileURL)
    }

    /// Removes every item the composer must not offer. Idempotent, so it can run more than once
    /// over the same menu.
    private func filter(_ menu: NSMenu) {
        for item in menu.items where !Self.allowed.contains(item.identifier?.rawValue ?? "") {
            menu.removeItem(item)
        }
    }

    /// Answers that nothing here can supply or receive a Services item, which is what keeps the
    /// **Services** submenu off the composer's menu.
    ///
    /// ⚠️ Filtering in `willOpenMenu` cannot reach it. AppKit builds Services while the menu is
    /// being displayed, after that call has returned, so the items are simply not there to remove
    /// and the menu the user sees is not the menu the filter saw. This is the hook that runs early
    /// enough, and it is what the submenu is assembled from.
    ///
    /// It has to go: the entries act on the selection, and they carry the draft out of the app.
    /// "Search With Google" would put a message the user is still writing on the network, which is
    /// gate 3 and gate 9 of docs/composer-security.md and the sovereignty rule behind them.
    override func validRequestor(
        forSendType sendType: NSPasteboard.PasteboardType?,
        returnType: NSPasteboard.PasteboardType?
    ) -> Any? {
        nil
    }
}
#endif
