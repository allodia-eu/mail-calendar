// Switching SwiftUI's accessibility on before the agent reads it, on macOS and iOS alike.
//
// A SwiftUI hosting view builds its accessibility elements only once an assistive technology has
// asked for them: until then it answers with its platform subviews alone. It tells such a request
// from an in-process call by asking the accessibility runtime which client is being served, and the
// agent is never one: a request a process sends itself is not served as a client's. What that first
// request does is set the hosting view's own `accessibilityEnabled`, so the agent sets it, through
// the setter SwiftUI exports, on each hosting view it has not set before. After that the view
// answers in process exactly as it answers VoiceOver.

#if DEBUG
import MailcalAgentShim
import ObjectiveC
#if os(macOS)
import AppKit
typealias AgentView = NSView
#else
import UIKit
typealias AgentView = UIView
#endif

@MainActor
enum AgentSwiftUI {
    #if os(macOS)
    private static let hostClass = "_TtGC7SwiftUI13NSHostingView"
    private static let setterSymbol = "$s7SwiftUI13NSHostingViewC20accessibilityEnabledSbvsTj"
    #else
    private static let hostClass = "_TtGC7SwiftUI14_UIHostingView"
    private static let setterSymbol = "$s7SwiftUI14_UIHostingViewC20accessibilityEnabledSbvsTj"
    #endif

    /// The hosting views already switched on, so each costs one call.
    private static let switchedOn = NSHashTable<AgentView>.weakObjects()

    /// The setter of the hosting view's `accessibilityEnabled`, found by its symbol: nil if SwiftUI
    /// no longer exports it, and the agent then says so rather than guessing.
    private static let setter: UnsafeMutableRawPointer? = dlsym(
        UnsafeMutableRawPointer(bitPattern: -2),  // RTLD_DEFAULT
        setterSymbol)

    private static var reported = false

    /// Whether a view is a SwiftUI hosting view, by class: a generic class has one name per content
    /// type, and every one of them starts with the generic's own.
    static func isHostingView(_ view: AgentView) -> Bool {
        var kind: AnyClass? = type(of: view)
        while let current = kind {
            if NSStringFromClass(current).hasPrefix(hostClass) { return true }
            kind = class_getSuperclass(current)
        }
        return false
    }

    /// Switches on the accessibility of every hosting view under `roots` that is not on yet.
    static func switchOn(under roots: [AgentView]) {
        guard let setter else {
            if !reported {
                reported = true
                TestAgent.say("SwiftUI exports no accessibility switch: SwiftUI views stay unread")
            }
            return
        }
        for view in roots.flatMap(hostingViews) where !switchedOn.contains(view) {
            mailcal_agent_set_bool(setter, Unmanaged.passUnretained(view).toOpaque(), true)
            switchedOn.add(view)
        }
    }

    private static func hostingViews(in root: AgentView) -> [AgentView] {
        guard !root.isHidden else { return [] }
        return (isHostingView(root) ? [root] : []) + root.subviews.flatMap(hostingViews)
    }
}
#endif
