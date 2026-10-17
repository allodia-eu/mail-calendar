// A recipient field's undo and redo (docs/undo.md, rule 9).
//
// The field's own, not the text input's. The input holds only the address being typed, and the
// field rewrites it whenever an address is finished, so the input's history would type a finished
// address back in and the field would finish it a second time. A step here is the whole field
// before a change: typing one address, finishing it, accepting a suggestion, removing a pill.

import Foundation

/// The values a field held before each change, and after each undo.
struct RecipientHistory {
    /// The most steps kept, oldest dropped first.
    static let maxSteps = 100

    private var undone: [String] = []
    private var redone: [String] = []
    /// Whether the newest step is an address still being typed, which the next keystroke joins.
    private var typing = false

    /// The value the field last held as far as this history knows.
    private(set) var current: String

    init(_ initial: String) {
        current = initial
    }

    var canUndo: Bool { !undone.isEmpty }
    var canRedo: Bool { !redone.isEmpty }

    /// Files a change to `after`. `typed` is a keystroke in the input, which joins the step before
    /// it while the same address is being typed; finishing it is a step of its own.
    mutating func record(_ after: String, typed: Bool) {
        guard after != current else { return }
        let sameRecipients = committedRecipients(current) == committedRecipients(after)
        if !(typed && typing && sameRecipients) {
            undone.append(current)
            if undone.count > Self.maxSteps { undone.removeFirst() }
        }
        redone.removeAll()
        typing = typed && sameRecipients
        current = after
    }

    /// The value to go back to, if there is one.
    mutating func undo() -> String? {
        guard let value = undone.popLast() else { return nil }
        redone.append(current)
        return moved(to: value)
    }

    /// The value an undo left behind, if nothing has changed since.
    mutating func redo() -> String? {
        guard let value = redone.popLast() else { return nil }
        undone.append(current)
        return moved(to: value)
    }

    /// Forgets everything: the field was given `value` from outside it, a reply's recipients say.
    mutating func restart(_ value: String) {
        undone.removeAll()
        redone.removeAll()
        typing = false
        current = value
    }

    private mutating func moved(to value: String) -> String {
        typing = false
        current = value
        return value
    }
}

/// A recipient field's history as the platform's Undo and Redo see it: the chords, the macOS Edit
/// menu, and on iPhone and iPad shaking, the three-finger gestures and the shortcut bar. The input
/// hands this out as its undo manager (`RecipientInput`), so whatever the input would file of its
/// own typing is kept nowhere.
final class RecipientUndoManager: UndoManager {
    var history = RecipientHistory("")
    /// Puts a value the history stepped to into the field.
    var apply: @MainActor (String) -> Void = { _ in }

    override var canUndo: Bool { history.canUndo }
    override var canRedo: Bool { history.canRedo }

    override func undo() {
        if let value = history.undo() { apply(value) }
    }

    override func redo() {
        if let value = history.redo() { apply(value) }
    }

    /// What shaking to undo calls rather than `undo()`. There are no nested groups.
    override func undoNestedGroup() {
        undo()
    }

    /// No name: the step is an address, and the menu reads plain Undo and Redo in the platform's
    /// own words.
    override var undoActionName: String { "" }
    override var redoActionName: String { "" }

    override func registerUndo(withTarget target: Any, selector: Selector, object anObject: Any?) {}
}
