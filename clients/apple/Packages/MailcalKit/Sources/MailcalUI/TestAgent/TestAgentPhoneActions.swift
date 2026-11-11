// The iOS half of the agent's acting: the accessibility actions VoiceOver performs (activate,
// increment, decrement), and for a text field the text input a keyboard types through. Never a
// synthesised touch or a coordinate.

#if DEBUG && os(iOS)
import UIKit

extension AgentPlatform {
    static func activate(_ object: NSObject) throws -> [String: Any] {
        if object.accessibilityActivate() { return ["how": "activated"] }
        // VoiceOver taps a control that does not activate itself; its own action is that tap's.
        if let control = object as? UIControl {
            let event: UIControl.Event = control.allControlEvents.contains(.primaryActionTriggered)
                ? .primaryActionTriggered : .touchUpInside
            control.sendActions(for: event)
            return ["how": "sent"]
        }
        throw AgentFailure(
            "the element does not activate: activate the control inside it, or one around it",
            "not-activatable"
        ).with("element", AgentTree.describe(object))
    }

    static func setText(_ object: NSObject, to text: String, submit: Bool) throws {
        guard let input = object as? (UIResponder & UITextInput) else {
            throw AgentFailure(
                "the element holds no text: query for role AXTextField or AXSearchField",
                "not-editable"
            ).with("element", AgentTree.describe(object))
        }
        if let view = input as? UITextView, !view.isEditable {
            throw AgentFailure("the element is read-only", "not-editable")
                .with("element", AgentTree.describe(object))
        }
        guard input.isFirstResponder || input.becomeFirstResponder() else {
            throw AgentFailure("the field does not take typing", "not-editable")
                .with("element", AgentTree.describe(object))
        }
        // Typed through the text input a keyboard uses, so the field's delegate and every binding
        // on it hear the change the way they hear a keystroke.
        input.selectedTextRange = input.textRange(
            from: input.beginningOfDocument, to: input.endOfDocument)
        if text.isEmpty {
            if input.hasText { input.deleteBackward() }
        } else {
            input.insertText(text)
        }
        // Return, as the keyboard sends it.
        if submit { input.insertText("\n") }
    }

    static func toggle(_ object: NSObject, to wanted: Bool?) throws -> Bool {
        guard role(of: object) == "AXSwitch" else {
            throw AgentFailure("the element has no on and off", "not-toggleable")
                .with("element", AgentTree.describe(object))
        }
        let current = isOn(object)
        guard (wanted ?? !current) != current else { return current }
        if let toggle = object as? UISwitch, !object.accessibilityActivate() {
            toggle.setOn(!current, animated: false)
            toggle.sendActions(for: .valueChanged)
        } else if !(object is UISwitch), !object.accessibilityActivate() {
            throw AgentFailure("the element did not take its activation", "not-toggleable")
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
        if let segments = object as? UISegmentedControl {
            try within(segments.numberOfSegments)
            segments.selectedSegmentIndex = index
            segments.sendActions(for: .valueChanged)
            return
        }
        guard role(of: object) == "AXTabGroup" else {
            throw AgentFailure("the element offers no choice", "not-selectable")
                .with("element", AgentTree.describe(object))
        }
        let choices = children(of: object).filter { $0.isAccessibilityElement }
        try within(choices.count)
        _ = try activate(choices[index])
    }

    /// VoiceOver's escape: offered to the element, then to each container out to its window, until
    /// one takes it.
    static func escape(from object: NSObject) throws -> String {
        var current: NSObject? = object
        for _ in 0..<128 {
            guard let here = current else { break }
            if here.accessibilityPerformEscape() { return "escaped" }
            current = (here as? UIView)?.superview
                ?? (here.responds(to: NSSelectorFromString("accessibilityContainer"))
                    ? here.value(forKey: "accessibilityContainer") as? NSObject : nil)
        }
        throw AgentFailure("nothing around the element takes an escape", "not-activatable")
    }

    /// VoiceOver's three-finger scroll, offered to the element and then each container out to its
    /// window, until one takes it.
    static func scroll(from object: NSObject, direction: String) throws {
        let way: UIAccessibilityScrollDirection = switch direction {
        case "up": .up
        case "down": .down
        case "left": .left
        default: .right
        }
        var current: NSObject? = object
        for _ in 0..<128 {
            guard let here = current else { break }
            if here.accessibilityScroll(way) { return }
            current = (here as? UIView)?.superview
                ?? (here.responds(to: NSSelectorFromString("accessibilityContainer"))
                    ? here.value(forKey: "accessibilityContainer") as? NSObject : nil)
        }
        throw AgentFailure("nothing around the element scrolls that way", "not-scrollable")
    }

    static func focus(_ object: NSObject) -> Bool {
        guard let responder = object as? UIResponder, responder.canBecomeFirstResponder else {
            return false
        }
        return responder.isFirstResponder || responder.becomeFirstResponder()
    }

    /// The accessibility increment or decrement of an adjustable element, and the value it left.
    static func step(_ object: NSObject, up: Bool) throws -> String? {
        guard object.accessibilityTraits.contains(.adjustable) else {
            throw AgentFailure("the element has no increment or decrement", "not-adjustable")
                .with("element", AgentTree.describe(object))
        }
        if up { object.accessibilityIncrement() } else { object.accessibilityDecrement() }
        return value(of: object)
    }

    /// Draws the window (or the element, cut from its window) into a PNG, from inside the app.
    static func screenshot(of object: NSObject?, to path: String) throws -> [String: Any] {
        guard let window = object.flatMap(window(of:)) ?? windows().first as? UIWindow else {
            throw AgentFailure("no window is open", "no-element")
        }
        let area = object.flatMap { $0 === window ? nil : bounds(of: $0) } ?? window.bounds
        let format = UIGraphicsImageRendererFormat(for: window.traitCollection)
        let picture = UIGraphicsImageRenderer(bounds: area, format: format).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: false)
        }
        guard let png = picture.pngData() else {
            throw AgentFailure("the picture could not be encoded", "no-element")
        }
        do {
            try png.write(to: URL(fileURLWithPath: path))
        } catch {
            throw AgentFailure.badRequest("cannot write \(path): \(error.localizedDescription)")
        }
        return [
            "path": path,
            "width": Int(area.width * format.scale), "height": Int(area.height * format.scale),
        ]
    }
}
#endif
