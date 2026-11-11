// The macOS half of the agent's acting: the accessibility actions VoiceOver performs, through the
// entry points the accessibility server calls, and for a text field the text system a person types
// through. Never a synthesised event or a coordinate.

#if DEBUG && os(macOS)
import AppKit

extension AgentPlatform {
    /// The roles a click chooses rather than presses.
    static let rowRoles: Set<NSAccessibility.Role> = [.row, .cell]

    static func activate(_ object: NSObject) throws -> [String: Any] {
        if perform("AXPress", on: object) { return ["how": "pressed"] }
        if perform("AXPick", on: object) { return ["how": "picked"] }
        // A row is chosen by a click on it, and a list row has no press of its own.
        if rowRoles.contains(kind(of: object)) {
            set("AXSelected", to: true, on: object)
            if flag(object, "AXSelected") == true { return ["how": "selected"] }
        }
        throw AgentFailure(
            "the element has no press: activate the control inside it, or one around it",
            "not-activatable"
        ).with("element", AgentTree.describe(object)).with("actions", actions(of: object))
    }

    static func setText(_ object: NSObject, to text: String, submit: Bool) throws {
        let readOnly = AgentFailure("the element is read-only", "not-editable")
            .with("element", AgentTree.describe(object))
        // A text field's element is often its cell; the field is what a person types into.
        if let field = object as? NSTextField ?? (object as? NSCell)?.controlView as? NSTextField {
            guard field.isEditable else { throw readOnly }
            // Typed through the field editor, so the field's delegate and every binding on it hear
            // the change the way they hear a keystroke.
            guard field.window?.makeFirstResponder(field) == true,
                  let editor = field.currentEditor() as? NSTextView
            else { throw readOnly }
            replace(in: editor, with: text)
            if submit { editor.doCommand(by: #selector(NSResponder.insertNewline(_:))) }
            return
        }
        if let view = object as? NSTextView {
            guard view.isEditable else { throw readOnly }
            view.window?.makeFirstResponder(view)
            replace(in: view, with: text)
            if submit { view.doCommand(by: #selector(NSResponder.insertNewline(_:))) }
            return
        }
        let editableRoles: Set<NSAccessibility.Role> = [.textField, .textArea, .comboBox]
        guard editableRoles.contains(kind(of: object)) else {
            throw AgentFailure(
                "the element holds no text: query for role AXTextField or AXTextArea",
                "not-editable"
            ).with("element", AgentTree.describe(object))
        }
        set(NSAccessibility.Attribute.value.rawValue, to: text, on: object)
        if submit, !perform("AXConfirm", on: object) { throw readOnly }
    }

    private static func replace(in editor: NSTextView, with text: String) {
        let whole = NSRange(location: 0, length: (editor.string as NSString).length)
        if text.isEmpty {
            editor.setSelectedRange(whole)
            editor.deleteBackward(nil)
        } else {
            editor.insertText(text, replacementRange: whole)
        }
    }

    static func toggle(_ object: NSObject, to wanted: Bool?) throws -> Bool {
        guard isSwitch(object) else {
            throw AgentFailure("the element has no on and off", "not-toggleable")
                .with("element", AgentTree.describe(object))
        }
        let current = isOn(object)
        if (wanted ?? !current) != current, !perform("AXPress", on: object) {
            throw AgentFailure("the element takes no press", "not-toggleable")
        }
        return isOn(object)
    }

    static func select(_ object: NSObject, index: Int) throws {
        let within = { (count: Int) in
            guard index < count else {
                throw AgentFailure(
                    "the element has \(count) choices, so no choice \(index)", "no-element")
            }
        }
        // A pop-up button chooses through its menu, as a person picking an item does.
        if let popUp = object as? NSPopUpButton {
            try within(popUp.numberOfItems)
            popUp.menu?.performActionForItem(at: index)
            return
        }
        switch kind(of: object) {
        case .radioGroup, .tabGroup:
            let choices = children(of: object).filter { kind(of: $0) == .radioButton }
            try within(choices.count)
            guard perform("AXPress", on: choices[index]) else {
                throw AgentFailure("the choice takes no press", "not-selectable")
            }
        case .list, .table, .outline:
            let rows = children(of: object).filter { rowRoles.contains(kind(of: $0)) }
            try within(rows.count)
            set("AXSelected", to: true, on: rows[index])
        default:
            throw AgentFailure("the element offers no choice", "not-selectable")
                .with("element", AgentTree.describe(object))
        }
    }

    /// VoiceOver's escape: the cancel action of the element or the nearest parent that has one,
    /// and otherwise what Escape sends along the responder chain.
    static func escape(from object: NSObject) throws -> String {
        var current: NSObject? = object
        for _ in 0..<128 {
            guard let here = current else { break }
            if perform("AXCancel", on: here) { return "cancelled" }
            current = attribute(here, NSAccessibility.Attribute.parent.rawValue) as? NSObject
        }
        if NSApplication.shared.sendAction(#selector(NSResponder.cancelOperation(_:)), to: nil, from: nil) {
            return "cancelled"
        }
        throw AgentFailure("nothing around the element takes an escape", "not-activatable")
    }

    /// A page of scrolling, as VoiceOver's scroll gesture gives it: the increment or decrement of
    /// the scroll bar of the element, or of the nearest scroll area around it.
    static func scroll(from object: NSObject, direction: String) throws {
        let vertical = direction == "up" || direction == "down"
        let action = direction == "up" || direction == "left" ? "AXDecrement" : "AXIncrement"
        var current: NSObject? = object
        for _ in 0..<128 {
            guard let here = current else { break }
            let bars = children(of: here).filter { kind(of: $0) == .scrollBar }
            if let bar = bars.first(where: {
                text($0, "AXOrientation") == (vertical ? "AXVerticalOrientation" : "AXHorizontalOrientation")
            }), perform(action, on: bar) {
                return
            }
            current = attribute(here, NSAccessibility.Attribute.parent.rawValue) as? NSObject
        }
        throw AgentFailure("nothing around the element scrolls that way", "not-scrollable")
    }

    static func focus(_ object: NSObject) -> Bool {
        if let view = object as? NSView, view.acceptsFirstResponder {
            view.window?.makeFirstResponder(view)
            if let field = view as? NSTextField { return field.currentEditor() != nil }
            return view.window?.firstResponder === view
        }
        set("AXFocused", to: true, on: object)
        return flag(object, "AXFocused") == true
    }

    /// The accessibility increment or decrement of a slider or stepper, and the value it left.
    static func step(_ object: NSObject, up: Bool) throws -> String? {
        guard perform(up ? "AXIncrement" : "AXDecrement", on: object) else {
            throw AgentFailure("the element has no increment or decrement", "not-adjustable")
                .with("element", AgentTree.describe(object))
        }
        return value(of: object)
    }

    /// Draws the window (or the element, cut from its window) into a PNG, from inside the app.
    static func screenshot(of object: NSObject?, to path: String) throws -> [String: Any] {
        guard let window = object.flatMap(window(of:)) ?? windows().first as? NSWindow,
              let frame = window.contentView?.superview ?? window.contentView
        else { throw AgentFailure("no window is open", "no-element") }
        guard let picture = frame.bitmapImageRepForCachingDisplay(in: frame.bounds) else {
            throw AgentFailure("the window could not be drawn", "no-element")
        }
        frame.cacheDisplay(in: frame.bounds, to: picture)
        guard var image = picture.cgImage else {
            throw AgentFailure("the window could not be drawn", "no-element")
        }
        if let object, object !== window, let bounds = bounds(of: object) {
            let scale = CGFloat(image.width) / frame.bounds.width
            let cut = CGRect(
                x: bounds.minX * scale, y: bounds.minY * scale,
                width: bounds.width * scale, height: bounds.height * scale
            ).integral
            guard let part = image.cropping(to: cut) else {
                throw AgentFailure("the element is not inside its window", "not-showing")
            }
            image = part
        }
        let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])
        guard let png else { throw AgentFailure("the picture could not be encoded", "no-element") }
        do {
            try png.write(to: URL(fileURLWithPath: path))
        } catch {
            throw AgentFailure.badRequest("cannot write \(path): \(error.localizedDescription)")
        }
        return ["path": path, "width": image.width, "height": image.height]
    }
}
#endif
