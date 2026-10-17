// Undo and Redo in the editor as the platform's own entry points see them (docs/undo.md, rule 3):
// the Edit menu on macOS, and on iPhone and iPad shaking, the three-finger gestures and the
// shortcut bar.
//
// On iPhone and iPad each of them asks the first responder's `undoManager` whether there is
// anything to undo, and tells it to undo; the macOS Edit menu asks the window's, and the web view
// takes its actions first (below). Left alone, both are where WebKit files the edits its engine
// makes: a stack the editor never uses, which has never heard of a table inserted or a list
// indented, and which offers Redo for undos it never ran. So the web view hands out this one
// instead, which says what the bundle's history says and steps that history.
//
// The bundle tells the host its state rather than being asked (`History` on the host channel),
// because both toolkits ask synchronously, and a script cannot answer within that call.

import WebKit

/// An `UndoManager` that is a view of the editor bundle's history, not a stack of its own.
final class EditorUndoManager: UndoManager {
    /// Runs `stepComposerHistory` in the page with `"undo"` or `"redo"`.
    var step: @MainActor (String) -> Void = { _ in }

    private var canStepBack = false
    private var canStepForward = false

    /// What the bundle last said it has to take back and to put back.
    func update(undo: Bool, redo: Bool) {
        canStepBack = undo
        canStepForward = redo
    }

    override var canUndo: Bool { canStepBack }
    override var canRedo: Bool { canStepForward }

    override func undo() {
        if canStepBack { step("undo") }
    }

    override func redo() {
        if canStepForward { step("redo") }
    }

    /// What shaking to undo calls rather than `undo()`. There are no nested groups.
    override func undoNestedGroup() {
        undo()
    }

    /// No name: a step in the body is whatever the user did, which the bundle does not label, so
    /// the menu reads plain Undo and Redo in the platform's own words.
    override var undoActionName: String { "" }
    override var redoActionName: String { "" }

    /// Where WebKit files the edits its engine makes. Kept nowhere: they are not the editor's
    /// history, and an undo of one would move nodes the editor's history points at.
    override func registerUndo(withTarget target: Any, selector: Selector, object anObject: Any?) {}
}

/// A `WKWebView` whose Undo and Redo are the editor's (`EditorUndoManager`). The composer's
/// `EditorWebView` is one, and so is the signature editor's view.
class HistoryWebView: WKWebView {
    let history = EditorUndoManager()

    override var undoManager: UndoManager? { history }

    #if os(macOS)
    // ⚠️ AppKit validates Edit → Undo and Redo against the **window's** undo manager, never the
    // first responder's, so the override above does not reach the menu. The web view is the first
    // responder, so it takes the two actions ahead of the window.
    @objc func undo(_ sender: Any?) {
        history.undo()
    }

    @objc func redo(_ sender: Any?) {
        history.redo()
    }

    override func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        switch item.action {
        case #selector(undo(_:)):
            (item as? NSMenuItem)?.title = history.undoMenuItemTitle
            return history.canUndo
        case #selector(redo(_:)):
            (item as? NSMenuItem)?.title = history.redoMenuItemTitle
            return history.canRedo
        default:
            return super.validateUserInterfaceItem(item)
        }
    }
    #endif
}
