// The macOS half of the agent's reading: NSAccessibility, asked in process on the main thread.
//
// Every attribute is read through `accessibilityAttributeValue:`, the entry point the accessibility
// server itself calls when VoiceOver asks, so what is read here is what VoiceOver is given. The
// typed protocol methods are no substitute: SwiftUI's elements answer some of them with an
// attributed string where a plain one is promised, and AppKit still answers a table's rows and a
// window's toolbar items only through the attribute.

#if DEBUG && os(macOS)
import AppKit

@MainActor
enum AgentPlatform {
    static func prepare() {}

    /// Every window on screen, the key window first.
    static func windows() -> [NSObject] {
        let shown = NSApplication.shared.windows.filter { $0.isVisible && !$0.isMiniaturized }
        return shown.filter(\.isKeyWindow) + shown.filter { !$0.isKeyWindow }
    }

    static func windowFacts(_ object: NSObject) -> [String: Any] {
        guard let window = object as? NSWindow else { return ["title": ""] }
        return [
            "title": window.title,
            "width": Int(window.frame.width),
            "height": Int(window.frame.height),
            "active": window.isKeyWindow,
            "modal": window.isModalPanel || window.isSheet,
        ]
    }

    /// One attribute, read as the accessibility server's accessor reads it for a client: through
    /// the element's own accessibility method when its class implements one, which is how SwiftUI's
    /// elements answer, and through `accessibilityAttributeValue:` when that gives nothing, which is
    /// how AppKit's tables and cells still answer (a cell takes its identifier from its control). Key-value coding calls the method, so a `BOOL` or a rectangle
    /// comes back boxed.
    static func attribute(_ object: NSObject, _ name: String) -> Any? {
        if let property = properties[name] {
            let getter = NSSelectorFromString(property.getter)
            if let own = class_getMethodImplementation(type(of: object), getter),
               own != class_getMethodImplementation(NSObject.self, getter),
               let value = object.value(forKey: property.key) {
                return value
            }
        }
        let selector = NSSelectorFromString("accessibilityAttributeValue:")
        guard object.responds(to: selector) else { return nil }
        return object.perform(selector, with: name)?.takeUnretainedValue()
    }

    /// Each attribute's method in the accessibility protocol, and its key for key-value coding.
    private static let properties: [String: (getter: String, key: String)] = [
        "AXRole": ("accessibilityRole", "accessibilityRole"),
        "AXSubrole": ("accessibilitySubrole", "accessibilitySubrole"),
        "AXTitle": ("accessibilityTitle", "accessibilityTitle"),
        "AXDescription": ("accessibilityLabel", "accessibilityLabel"),
        "AXHelp": ("accessibilityHelp", "accessibilityHelp"),
        "AXIdentifier": ("accessibilityIdentifier", "accessibilityIdentifier"),
        "AXValue": ("accessibilityValue", "accessibilityValue"),
        "AXEnabled": ("isAccessibilityEnabled", "accessibilityEnabled"),
        "AXFocused": ("isAccessibilityFocused", "accessibilityFocused"),
        "AXSelected": ("isAccessibilitySelected", "accessibilitySelected"),
        "AXExpanded": ("isAccessibilityExpanded", "accessibilityExpanded"),
        "AXParent": ("accessibilityParent", "accessibilityParent"),
        "AXWindow": ("accessibilityWindow", "accessibilityWindow"),
        "AXFrame": ("accessibilityFrame", "accessibilityFrame"),
    ]

    static func text(_ object: NSObject, _ name: String) -> String? {
        string(attribute(object, name))
    }

    static func flag(_ object: NSObject, _ name: String) -> Bool? {
        (attribute(object, name) as? NSNumber)?.boolValue
    }

    /// The children the server hands a client.
    ///
    /// AppKit's views answer with their children already expanded past the elements they ignore,
    /// and out of a client's request that expansion reads a SwiftUI hosting view's AppKit subviews,
    /// never its SwiftUI elements. So a child found inside a hosting view is replaced here by that
    /// hosting view's own elements, which is what a client's request is given.
    static func children(of object: NSObject) -> [NSObject] {
        let view = (object as? NSWindow).map { $0.contentView?.superview ?? $0.contentView }
            ?? (object as? NSView)
        var children: [Any] = []
        var swapped = Set<ObjectIdentifier>()
        for child in ownChildren(of: object) {
            guard let view, let host = outermostHost(of: child, below: view) else {
                children.append(child)
                continue
            }
            if swapped.insert(ObjectIdentifier(host)).inserted {
                children += host.accessibilityChildren() ?? []
            }
        }
        var seen = Set<ObjectIdentifier>()
        return NSAccessibility.unignoredChildren(from: children).compactMap { $0 as? NSObject }
            .filter { seen.insert(ObjectIdentifier($0)).inserted }
    }

    /// Read as the server's accessor reads them: the element's own `accessibilityChildren` first,
    /// and the attribute when that is empty, which is where AppKit's tables answer with their rows.
    private static func ownChildren(of object: NSObject) -> [Any] {
        if let view = object as? NSView, isHostingView(view) {
            return view.accessibilityChildren() ?? []
        }
        let own = NSSelectorFromString("accessibilityChildren")
        let children = object.responds(to: own)
            ? object.perform(own)?.takeUnretainedValue() as? [Any] ?? [] : []
        if !children.isEmpty { return children }
        return attribute(object, NSAccessibility.Attribute.children.rawValue) as? [Any] ?? []
    }

    /// The hosting view nearest `top` that holds `child`, if one does below `top`.
    private static func outermostHost(of child: Any, below top: NSView) -> NSView? {
        var view = child as? NSView ?? (child as? NSCell)?.controlView
        var host: NSView?
        while let current = view, current !== top {
            if isHostingView(current) { host = current }
            view = current.superview
        }
        return view === top ? host : nil
    }

    static func isHostingView(_ view: NSView) -> Bool {
        AgentSwiftUI.isHostingView(view)
    }

    static func awaken() async {
        AgentSwiftUI.switchOn(under: windows().compactMap {
            ($0 as? NSWindow).flatMap { $0.contentView?.superview ?? $0.contentView }
        })
    }

    static func role(of object: NSObject) -> String {
        text(object, NSAccessibility.Attribute.role.rawValue) ?? "AXUnknown"
    }

    static func kind(of object: NSObject) -> NSAccessibility.Role {
        NSAccessibility.Role(rawValue: role(of: object))
    }

    static func texts(of object: NSObject) -> [String] {
        var texts = [
            text(object, NSAccessibility.Attribute.title.rawValue),
            text(object, NSAccessibility.Attribute.description.rawValue),
        ]
        // A static text's words are its value, and what a person reads.
        if kind(of: object) == .staticText {
            texts.append(text(object, NSAccessibility.Attribute.value.rawValue))
        }
        var seen = Set<String>()
        return texts.compactMap { $0 }.filter { seen.insert($0).inserted }
    }

    static func identifier(of object: NSObject) -> String? {
        // A control's element is often its cell, and the identifier is set on the control.
        text(object, NSAccessibility.Attribute.identifier.rawValue)
            ?? (object as? NSCell)?.controlView.flatMap { text($0, NSAccessibility.Attribute.identifier.rawValue) }
    }

    static func tooltip(of object: NSObject) -> String? {
        text(object, NSAccessibility.Attribute.help.rawValue)
    }

    static func value(of object: NSObject) -> String? {
        guard kind(of: object) != .staticText else { return nil }
        return text(object, NSAccessibility.Attribute.value.rawValue)
    }

    static func isEnabled(_ object: NSObject) -> Bool {
        if let window = object as? NSWindow { return window.isVisible }
        // An element that does not say, a list row for one, is one a person can act on.
        return flag(object, NSAccessibility.Attribute.enabled.rawValue) ?? true
    }

    /// The window an element is drawn in.
    static func window(of object: NSObject) -> NSWindow? {
        var current: NSObject? = object
        for _ in 0..<128 {
            guard let here = current else { return nil }
            if let window = here as? NSWindow { return window }
            if let view = here as? NSView { return view.window }
            if let window = attribute(here, NSAccessibility.Attribute.window.rawValue) as? NSWindow {
                return window
            }
            current = attribute(here, NSAccessibility.Attribute.parent.rawValue) as? NSObject
        }
        return nil
    }

    static func isAttached(_ object: NSObject) -> Bool {
        window(of: object)?.isVisible == true
    }

    /// The element's frame in its window, top left first, in points.
    static func bounds(of object: NSObject) -> CGRect? {
        guard let window = window(of: object) else { return nil }
        if object === window { return CGRect(origin: .zero, size: window.frame.size) }
        // Cocoa's screen space, as AppKit keeps it before the server turns it over for a client:
        // the position is the bottom left corner, and y grows upwards.
        let frame: CGRect
        if let rect = (attribute(object, "AXFrame") as? NSValue)?.rectValue {
            frame = rect
        } else if let origin = (attribute(object, "AXPosition") as? NSValue)?.pointValue,
                  let size = (attribute(object, "AXSize") as? NSValue)?.sizeValue {
            frame = CGRect(origin: origin, size: size)
        } else {
            return nil
        }
        return CGRect(
            x: frame.minX - window.frame.minX, y: window.frame.maxY - frame.maxY,
            width: frame.width, height: frame.height)
    }

    static func isShowing(_ object: NSObject, bounds: CGRect?) -> Bool {
        guard let window = window(of: object), window.isVisible, !window.isMiniaturized,
              let bounds, bounds.width > 0, bounds.height > 0
        else { return false }
        return bounds.intersects(CGRect(origin: .zero, size: window.frame.size))
    }

    /// The roles whose value says whether they are on.
    static let toggleRoles: Set<NSAccessibility.Role> = [.checkBox, .radioButton, .disclosureTriangle]

    static func isSwitch(_ object: NSObject) -> Bool {
        toggleRoles.contains(kind(of: object))
            || text(object, NSAccessibility.Attribute.subrole.rawValue) == "AXSwitch"
    }

    static func isOn(_ object: NSObject) -> Bool {
        (attribute(object, NSAccessibility.Attribute.value.rawValue) as? NSNumber)?.intValue == 1
    }

    static func state(of object: NSObject, role: String) -> [String: Bool] {
        var state: [String: Bool] = [:]
        if isSwitch(object) {
            state[kind(of: object) == .disclosureTriangle ? "expanded" : "active"] = isOn(object)
        }
        for (name, key) in [("AXFocused", "focused"), ("AXSelected", "selected"), ("AXExpanded", "expanded")]
        where flag(object, name) == true {
            state[key] = true
        }
        if let editable = editable(object) { state["readOnly"] = !editable }
        return state
    }

    /// Whether a text control takes typing, when the element is one: nil for anything else.
    static func editable(_ object: NSObject) -> Bool? {
        if let field = object as? NSTextField ?? (object as? NSCell)?.controlView as? NSTextField {
            return field.isEditable
        }
        if let view = object as? NSTextView { return view.isEditable }
        return nil
    }

    /// Whether the element's class implements `selector` itself, rather than inheriting NSObject's
    /// mapping onto the attribute API: what the server's accessor asks before choosing a path.
    private static func implements(_ object: NSObject, _ selector: Selector) -> Bool {
        guard let own = class_getMethodImplementation(type(of: object), selector) else { return false }
        return own != class_getMethodImplementation(NSObject.self, selector)
    }

    /// Each action's method in the accessibility protocol.
    private static let actionMethods: [String: String] = [
        "AXPress": "accessibilityPerformPress", "AXPick": "accessibilityPerformPick",
        "AXIncrement": "accessibilityPerformIncrement",
        "AXDecrement": "accessibilityPerformDecrement",
        "AXConfirm": "accessibilityPerformConfirm", "AXCancel": "accessibilityPerformCancel",
        "AXShowMenu": "accessibilityPerformShowMenu",
    ]

    /// The actions the element offers a client: `AXPress`, `AXIncrement`, `AXShowMenu`.
    static func actions(of object: NSObject) -> [String] {
        let own = actionMethods.filter { implements(object, NSSelectorFromString($0.value)) }.keys
        let selector = NSSelectorFromString("accessibilityActionNames")
        let legacy = object.responds(to: selector)
            ? object.perform(selector)?.takeUnretainedValue() as? [String] ?? [] : []
        return Array(Set(own).union(legacy)).sorted()
    }

    /// Performs one of the element's actions as a client's request does. Answers whether the element
    /// took it.
    static func perform(_ action: String, on object: NSObject) -> Bool {
        if let name = actionMethods[action] {
            let selector = NSSelectorFromString(name)
            if implements(object, selector), let method = object.method(for: selector) {
                typealias Perform = @convention(c) (AnyObject, Selector) -> Bool
                let taken = unsafeBitCast(method, to: Perform.self)(object, selector)
                // A cell performs the action and answers no all the same, so its answer says
                // nothing, and a second try through the attribute API would act twice.
                if taken || object is NSCell { return true }
            }
        }
        let selector = NSSelectorFromString("accessibilityActionNames")
        guard object.responds(to: selector),
              (object.perform(selector)?.takeUnretainedValue() as? [String])?.contains(action) == true
        else { return false }
        _ = object.perform(NSSelectorFromString("accessibilityPerformAction:"), with: action)
        return true
    }

    /// Sets an attribute as a client's request does: through the element's own setter when its
    /// class implements one, and the attribute API otherwise.
    static func set(_ name: String, to value: Any, on object: NSObject) {
        if let property = properties[name] {
            let key = property.key
            let setter = NSSelectorFromString("set\(key.prefix(1).uppercased())\(key.dropFirst()):")
            if implements(object, setter) {
                object.setValue(value, forKey: key)
                return
            }
        }
        _ = object.perform(
            NSSelectorFromString("accessibilitySetValue:forAttribute:"), with: value, with: name)
    }

    private static func string(_ value: Any?) -> String? {
        let text: String? = switch value {
        case let text as String: text
        case let text as NSAttributedString: text.string
        case let number as NSNumber: number.stringValue
        default: nil
        }
        guard let text, !text.isEmpty else { return nil }
        return text
    }

    static func quit() {
        NSApplication.shared.terminate(nil)
    }
}
#endif
