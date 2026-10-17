// The text input inside a recipient field, the platform's own text field rather than SwiftUI's,
// because its undo has to be the field's (docs/undo.md, rule 9) and a SwiftUI `TextField` offers no
// way to say so. Left alone, its undo is the window's, where it files its own typing: ⌘Z after an
// address was finished typed the address back in, and the field finished it a second time.
//
// So the input hands out the field's `RecipientUndoManager` as its undo manager, and what it would
// file of its own typing is kept nowhere. On iPhone and iPad that is the whole of it: the chords,
// shaking, the three-finger gestures and the shortcut bar all ask the first responder's undo
// manager, and the text field is the first responder. On macOS the Edit menu asks the window's, so
// the field editor also takes `undo:` and `redo:` itself, as the message body's web view does.

import SwiftUI

#if os(macOS)
import AppKit

struct RecipientInput: NSViewRepresentable {
    let label: String
    let text: String
    /// Told whether the field has the keyboard.
    @Binding var isFocused: Bool
    /// Raised to put the keyboard in the field: a new value is a request, so asking twice works
    /// where setting `isFocused` to what it already says would not.
    let focusRequest: Int
    let undo: RecipientUndoManager
    /// Told what the user made the text, keystroke by keystroke; a value set from outside is not
    /// an edit.
    let onEdit: (String) -> Void
    /// Return: `true` when the field took it, `false` to leave it to the platform.
    let onReturn: () -> Bool
    /// Ignored: a macOS text field is drawn with its rounded border wherever this one is used.
    var bordered = false

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.cell = RecipientInputCell(undo: undo)
        field.isEditable = true
        field.isBezeled = true
        field.bezelStyle = .roundedBezel
        field.font = .systemFont(ofSize: NSFont.systemFontSize)
        field.placeholderString = label
        field.setAccessibilityLabel(label)
        field.cell?.isScrollable = true
        field.cell?.usesSingleLineMode = true
        field.isAutomaticTextCompletionEnabled = false
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.delegate = context.coordinator
        return field
    }

    func updateNSView(_ field: NSTextField, context: Context) {
        context.coordinator.parent = self
        // Only a value the field has not already shown. An update can arrive between a keystroke and
        // the field reporting it, carrying the text from before it, which would undo the keystroke.
        if text != context.coordinator.shown {
            context.coordinator.shown = text
            field.stringValue = text
            // A rewrite while editing selects the whole text, which the next keystroke would
            // replace. The caret goes to the end, as after typing.
            field.currentEditor()?.selectedRange = NSRange(location: (text as NSString).length, length: 0)
        }
        if focusRequest != context.coordinator.focusRequest, let window = field.window {
            context.coordinator.focusRequest = focusRequest
            // After this update, never during it: becoming first responder reports focus back.
            DispatchQueue.main.async {
                if field.currentEditor() == nil { window.makeFirstResponder(field) }
            }
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: RecipientInput
        /// What the field last showed: what the user typed, or what it was last given.
        var shown = ""
        /// The newest focus request acted on.
        var focusRequest = 0

        init(_ parent: RecipientInput) { self.parent = parent }

        func controlTextDidBeginEditing(_ notification: Notification) {
            if !parent.isFocused { parent.isFocused = true }
        }

        func controlTextDidEndEditing(_ notification: Notification) {
            if parent.isFocused { parent.isFocused = false }
        }

        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            shown = field.stringValue
            parent.onEdit(field.stringValue)
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
            selector == #selector(NSResponder.insertNewline(_:)) && parent.onReturn()
        }
    }
}

/// Edits the field in a field editor of its own, which answers Undo and Redo from the field's
/// history.
private final class RecipientInputCell: NSTextFieldCell {
    /// AppKit's own field editor, configured as it configures one, with Undo and Redo the field's.
    private let editor = RecipientFieldEditor.fieldEditor()

    init(undo: RecipientUndoManager) {
        editor.history = undo
        super.init(textCell: "")
    }

    required init(coder: NSCoder) {
        fatalError("not built from a nib")
    }

    override func fieldEditor(for controlView: NSView) -> NSTextView? {
        editor
    }
}

private final class RecipientFieldEditor: NSTextView {
    var history: RecipientUndoManager?

    override var undoManager: UndoManager? { history }

    @objc func undo(_ sender: Any?) {
        history?.undo()
    }

    @objc func redo(_ sender: Any?) {
        history?.redo()
    }

    override func validateUserInterfaceItem(_ item: any NSValidatedUserInterfaceItem) -> Bool {
        guard let history else { return super.validateUserInterfaceItem(item) }
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
}

#else
import UIKit

struct RecipientInput: UIViewRepresentable {
    let label: String
    let text: String
    /// Told whether the field has the keyboard.
    @Binding var isFocused: Bool
    /// Raised to put the keyboard in the field: a new value is a request, so asking twice works
    /// where setting `isFocused` to what it already says would not.
    let focusRequest: Int
    let undo: RecipientUndoManager
    /// Told what the user made the text, keystroke by keystroke; a value set from outside is not
    /// an edit.
    let onEdit: (String) -> Void
    /// Return: `true` when the field took it, `false` to leave it to the platform.
    let onReturn: () -> Bool
    /// Drawn with a rounded border, as `.textFieldStyle(.roundedBorder)` would draw SwiftUI's.
    var bordered = false

    func makeUIView(context: Context) -> RecipientTextField {
        let field = RecipientTextField()
        field.undo = undo
        field.placeholder = label
        field.accessibilityLabel = label
        field.borderStyle = bordered ? .roundedRect : .none
        field.font = .preferredFont(forTextStyle: .body)
        field.adjustsFontForContentSizeCategory = true
        // The email keyboard, with `@` on its first page, and nothing "corrected" in an address.
        field.keyboardType = .emailAddress
        field.textContentType = .emailAddress
        field.autocapitalizationType = .none
        field.autocorrectionType = .no
        field.spellCheckingType = .no
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.delegate = context.coordinator
        field.addTarget(context.coordinator, action: #selector(Coordinator.changed(_:)), for: .editingChanged)
        return field
    }

    func updateUIView(_ field: RecipientTextField, context: Context) {
        context.coordinator.parent = self
        // Only a value the field has not already shown. An update can arrive between a keystroke and
        // the field reporting it, carrying the text from before it, which would undo the keystroke.
        if text != context.coordinator.shown {
            context.coordinator.shown = text
            field.text = text
        }
        if focusRequest != context.coordinator.focusRequest, field.window != nil {
            context.coordinator.focusRequest = focusRequest
            // After this update, never during it: becoming first responder reports focus back.
            DispatchQueue.main.async {
                if !field.isFirstResponder { field.becomeFirstResponder() }
            }
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    @MainActor
    final class Coordinator: NSObject, UITextFieldDelegate {
        var parent: RecipientInput
        /// What the field last showed: what the user typed, or what it was last given.
        var shown = ""
        /// The newest focus request acted on.
        var focusRequest = 0

        init(_ parent: RecipientInput) { self.parent = parent }

        @objc func changed(_ field: UITextField) {
            shown = field.text ?? ""
            parent.onEdit(shown)
        }

        func textFieldDidBeginEditing(_ textField: UITextField) {
            if !parent.isFocused { parent.isFocused = true }
        }

        func textFieldDidEndEditing(_ textField: UITextField) {
            if parent.isFocused { parent.isFocused = false }
        }

        /// Return the field takes keeps the keyboard up for the next address; one it leaves puts
        /// the keyboard away, as a SwiftUI field does.
        func textFieldShouldReturn(_ textField: UITextField) -> Bool {
            if !parent.onReturn() { textField.resignFirstResponder() }
            return false
        }
    }
}

final class RecipientTextField: UITextField {
    var undo: RecipientUndoManager?

    override var undoManager: UndoManager? { undo }
}
#endif
