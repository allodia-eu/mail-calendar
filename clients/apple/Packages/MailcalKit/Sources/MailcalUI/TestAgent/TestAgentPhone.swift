// The iOS half of the agent's reading: UIAccessibility, asked in process on the main thread. An
// element's children are its `accessibilityElements`, or a container's numbered elements, or a
// view's subviews, in that order, as VoiceOver reads them; an element hides what is inside it.
//
// iOS names no role, only traits, so a role is spelt from the traits in the words the accessibility
// runtime gives XCUITest and idb (`AXButton`, `AXStaticText`, `AXSearchField`), which is also how
// the macOS half spells them.

#if DEBUG && os(iOS)
import UIKit

/// The simulator's automation switch as the agent found it, put back as the app exits.
nonisolated(unsafe) private var agentAutomationBefore: Int32 = 0

@MainActor
enum AgentPlatform {
    /// Turns on the accessibility runtime, as XCUITest does, so UIKit loads the accessibility
    /// bundles that name its own controls (a tab button, a navigation title) for VoiceOver. Without
    /// them those controls answer with no label at all. The switch is the simulator's, not the
    /// app's, and the agent puts it back as the app exits.
    static func prepare() {
        guard let library = dlopen("/usr/lib/libAccessibility.dylib", RTLD_NOW),
              let get = dlsym(library, "_AXSAutomationEnabled"),
              let set = dlsym(library, "_AXSSetAutomationEnabled")
        else {
            TestAgent.say("no accessibility runtime to turn on: UIKit's controls stay unnamed")
            return
        }
        typealias Get = @convention(c) () -> Int32
        typealias Set = @convention(c) (Int32) -> Void
        let before = unsafeBitCast(get, to: Get.self)()
        unsafeBitCast(set, to: Set.self)(1)
        agentAutomationBefore = before
        atexit {
            guard let set = dlsym(dlopen("/usr/lib/libAccessibility.dylib", RTLD_NOW),
                                  "_AXSSetAutomationEnabled")
            else { return }
            unsafeBitCast(set, to: (@convention(c) (Int32) -> Void).self)(agentAutomationBefore)
        }
    }

    static func awaken() async {
        AgentSwiftUI.switchOn(under: windows().compactMap { $0 as? UIView })
    }

    /// Every window on screen, the key window first.
    static func windows() -> [NSObject] {
        let shown = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows).filter { !$0.isHidden && $0.alpha > 0 }
        return shown.filter(\.isKeyWindow) + shown.filter { !$0.isKeyWindow }
    }

    static func windowFacts(_ object: NSObject) -> [String: Any] {
        guard let window = object as? UIWindow else { return ["title": ""] }
        return [
            "title": window.windowScene?.title ?? "",
            "width": Int(window.bounds.width),
            "height": Int(window.bounds.height),
            "active": window.isKeyWindow,
            "modal": false,
        ]
    }

    static func children(of object: NSObject) -> [NSObject] {
        if !(object is UIWindow), object.isAccessibilityElement { return [] }
        if object.accessibilityElementsHidden { return [] }
        if let elements = object.accessibilityElements, !elements.isEmpty {
            return elements.compactMap { $0 as? NSObject }
        }
        let count = object.accessibilityElementCount()
        if count != NSNotFound, count > 0 {
            return (0..<count).compactMap { object.accessibilityElement(at: $0) as? NSObject }
        }
        guard let view = object as? UIView else { return [] }
        return view.subviews.filter { !$0.isHidden && $0.alpha > 0.01 }
    }

    static func role(of object: NSObject) -> String {
        let traits = object.accessibilityTraits
        if object is UIWindow { return "AXWindow" }
        if traits.contains(.searchField) || object is UISearchTextField { return "AXSearchField" }
        if object is UITextField { return "AXTextField" }
        if object is UITextView { return "AXTextArea" }
        if object is UISwitch || traits.contains(.toggleButton) { return "AXSwitch" }
        if traits.contains(.adjustable) { return "AXSlider" }
        if traits.contains(.tabBar) { return "AXTabGroup" }
        if traits.contains(.link) { return "AXLink" }
        if traits.contains(.button) || traits.contains(.keyboardKey) { return "AXButton" }
        if traits.contains(.header) { return "AXHeading" }
        if traits.contains(.image) { return "AXImage" }
        if traits.contains(.staticText) { return "AXStaticText" }
        return object.isAccessibilityElement ? "AXElement" : "AXGroup"
    }

    static func texts(of object: NSObject) -> [String] {
        [object.accessibilityLabel].compactMap { $0 }.filter { !$0.isEmpty }
    }

    static func identifier(of object: NSObject) -> String? {
        let id = (object as? UIAccessibilityIdentification)?.accessibilityIdentifier
        return id?.isEmpty == false ? id : nil
    }

    /// iOS has no tooltip; the hint is the help VoiceOver reads after a pause.
    static func tooltip(of object: NSObject) -> String? {
        let hint = object.accessibilityHint
        return hint?.isEmpty == false ? hint : nil
    }

    static func value(of object: NSObject) -> String? {
        if let field = object as? UITextField { return field.text ?? "" }
        if let view = object as? UITextView { return view.text }
        let value = object.accessibilityValue
        return value?.isEmpty == false ? value : nil
    }

    static func isEnabled(_ object: NSObject) -> Bool {
        if let control = object as? UIControl, !control.isEnabled { return false }
        return !object.accessibilityTraits.contains(.notEnabled)
    }

    /// The window an element is drawn in, found through its containers.
    static func window(of object: NSObject) -> UIWindow? {
        var current: Any? = object
        for _ in 0..<128 {
            switch current {
            case let window as UIWindow: return window
            case let view as UIView: return view.window
            case let element as UIAccessibilityElement: current = element.accessibilityContainer
            case let other as NSObject:
                let container = NSSelectorFromString("accessibilityContainer")
                current = other.responds(to: container) ? other.value(forKey: "accessibilityContainer") : nil
            default: return nil
            }
        }
        return nil
    }

    static func isAttached(_ object: NSObject) -> Bool {
        guard let window = window(of: object) else { return false }
        return !window.isHidden && window.windowScene != nil
    }

    /// The element's frame in its window, in points.
    static func bounds(of object: NSObject) -> CGRect? {
        guard let window = window(of: object) else { return nil }
        if object === window { return window.bounds }
        // A view's own geometry: its `accessibilityFrame` is filled in only by the accessibility
        // runtime, which is not loaded unless an assistive technology runs.
        if let view = object as? UIView { return view.convert(view.bounds, to: window) }
        let frame = object.accessibilityFrame
        guard !frame.isNull, let screen = window.windowScene?.screen else { return nil }
        return window.convert(frame, from: screen.coordinateSpace)
    }

    static func isShowing(_ object: NSObject, bounds: CGRect?) -> Bool {
        guard let window = window(of: object), !window.isHidden,
              let bounds, bounds.width > 0, bounds.height > 0
        else { return false }
        var view = object as? UIView
        while let current = view {
            if current.isHidden || current.alpha < 0.01 { return false }
            view = current.superview
        }
        return bounds.intersects(window.bounds)
    }

    static func isOn(_ object: NSObject) -> Bool {
        if let toggle = object as? UISwitch { return toggle.isOn }
        return object.accessibilityValue == "1"
    }

    static func state(of object: NSObject, role: String) -> [String: Bool] {
        var state: [String: Bool] = [:]
        if role == "AXSwitch" { state["active"] = isOn(object) }
        if object.accessibilityTraits.contains(.selected) { state["selected"] = true }
        if (object as? UIResponder)?.isFirstResponder == true { state["focused"] = true }
        if let editable = editable(object) { state["readOnly"] = !editable }
        return state
    }

    /// Whether a text control takes typing, when the element is one: nil for anything else.
    static func editable(_ object: NSObject) -> Bool? {
        // A field a person cannot type into is disabled: UIKit has no read-only text field.
        if object is UITextField { return true }
        if let view = object as? UITextView { return view.isEditable }
        return nil
    }

    /// iOS apps have no Quit; the process ends, as `simctl terminate` ends it.
    static func quit() {
        exit(0)
    }
}
#endif
