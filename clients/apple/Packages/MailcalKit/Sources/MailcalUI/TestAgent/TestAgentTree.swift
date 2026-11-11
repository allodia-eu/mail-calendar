// The accessibility tree as the agent reports it: handles, what an element says, and the indented
// dump. What each field holds comes from the platform half (TestAgentMac, TestAgentPhone).

#if DEBUG
import CoreGraphics
import Foundation

/// One accessibility element, read once per lookup: each property asks the platform at most once.
@MainActor
final class AgentNode {
    let object: NSObject

    init(_ object: NSObject) {
        self.object = object
    }

    lazy var role: String = AgentPlatform.role(of: object)
    lazy var texts: [String] = AgentPlatform.texts(of: object)
    lazy var identifier: String? = AgentPlatform.identifier(of: object)
    lazy var tooltip: String? = AgentPlatform.tooltip(of: object)
    lazy var value: String? = AgentPlatform.value(of: object)
    lazy var enabled: Bool = AgentPlatform.isEnabled(object)
    lazy var bounds: CGRect? = AgentPlatform.bounds(of: object)
    lazy var showing: Bool = AgentPlatform.isShowing(object, bounds: bounds)
    lazy var state: [String: Bool] = AgentPlatform.state(of: object, role: role)

    var type: String { String(describing: Swift.type(of: object)) }

    /// Everything a filter's text is compared with.
    var said: [String] { texts + [tooltip, value].compactMap { $0 } }

    /// Whether the element's class, or one it derives from, is named `name`.
    func isKind(of name: String) -> Bool {
        var kind: AnyClass? = Swift.type(of: object)
        while let current = kind {
            if String(describing: current) == name || NSStringFromClass(current) == name {
                return true
            }
            kind = class_getSuperclass(current)
        }
        return false
    }
}

@MainActor
enum AgentTree {
    /// Handles stay the same for an element's whole life, so a caller can query once and act later.
    private static var next = 0
    private static var byObject: [ObjectIdentifier: Int] = [:]
    private static var objects: [Int: Weak] = [:]

    /// How many handles are handed out between sweeps of the elements that have gone.
    private static let sweepEvery = 4096

    /// The elements the last few replies named, held so a caller can act on what it was just
    /// shown: AppKit makes a table's row and cell elements afresh for each request and keeps none.
    private static var recent: [[NSObject]] = [[]]

    /// Starts a request: what the reply before last named may go.
    static func beginRequest() {
        recent = [recent.last ?? [], []]
    }

    private final class Weak {
        weak var object: NSObject?
        init(_ object: NSObject) { self.object = object }
    }

    static func handle(_ object: NSObject) -> Int {
        let key = ObjectIdentifier(object)
        if let h = byObject[key], objects[h]?.object === object {
            recent[recent.count - 1].append(object)
            return h
        }
        next += 1
        // `settle` names every element on screen each poll, so one that went is let go here rather
        // than kept for the life of the app.
        if next.isMultiple(of: sweepEvery) {
            objects = objects.filter { $0.value.object != nil }
            byObject = byObject.filter { objects[$0.value] != nil }
        }
        byObject[key] = next
        objects[next] = Weak(object)
        recent[recent.count - 1].append(object)
        return next
    }

    /// The element a handle names, or why there is none.
    static func element(_ h: Int) throws -> NSObject {
        guard let weak = objects[h] else {
            // A handle that was handed out and is no longer kept named an element that went.
            if h >= 1 && h <= next {
                throw AgentFailure("element \(h) is gone: query again", "gone")
            }
            throw AgentFailure("no element \(h)", "no-element")
        }
        guard let object = weak.object, AgentPlatform.isAttached(object) else {
            throw AgentFailure("element \(h) is gone: query again", "gone")
        }
        return object
    }

    /// The children of an element in the order assistive technology reads them.
    static func children(of object: NSObject) -> [NSObject] {
        AgentPlatform.children(of: object)
    }

    /// Visits `object` and its descendants depth first, each with its ancestors nearest last,
    /// until `visit` answers false.
    @discardableResult
    static func walk(
        _ object: NSObject, ancestors: [NSObject] = [],
        _ visit: (NSObject, [NSObject]) -> Bool
    ) -> Bool {
        // An accessibility tree is the app's own claim about itself; a cycle in it must not hang
        // the agent.
        guard ancestors.count < 128, !ancestors.contains(where: { $0 === object }) else {
            return true
        }
        guard visit(object, ancestors) else { return false }
        let path = ancestors + [object]
        for child in children(of: object) where !walk(child, ancestors: path, visit) {
            return false
        }
        return true
    }

    static func windows() -> [String: Any] {
        ["windows": AgentPlatform.windows().map { window in
            var facts = AgentPlatform.windowFacts(window)
            facts["h"] = handle(window)
            facts["type"] = String(describing: type(of: window))
            return facts
        }]
    }

    static func describe(_ object: NSObject) -> [String: Any] {
        describe(AgentNode(object))
    }

    static func describe(_ node: AgentNode) -> [String: Any] {
        var described: [String: Any] = [
            "h": handle(node.object),
            "type": node.type,
            "role": node.role,
            "showing": node.showing,
            "enabled": node.enabled,
        ]
        if !node.texts.isEmpty { described["text"] = node.texts }
        if let id = node.identifier { described["id"] = id }
        if let tooltip = node.tooltip { described["tooltip"] = tooltip }
        if let value = node.value { described["value"] = value }
        if let bounds = node.bounds {
            // Points in the window's own space, top left first.
            described["bounds"] = [bounds.minX, bounds.minY, bounds.width, bounds.height]
                .map { Int($0.rounded()) }
        }
        for (key, value) in node.state { described[key] = value }
        return described
    }

    /// One element as a dump line: `[12] NSButton #id "Sync" AXButton value="…" (disabled)`.
    /// Without its handle, the line is what `settle` compares from one poll to the next.
    static func summary(_ node: AgentNode, handle withHandle: Bool = true) -> String {
        var line = withHandle ? "[\(handle(node.object))] \(node.type)" : node.type
        if let id = node.identifier { line += " #\(id)" }
        for text in node.texts { line += " \(quoted(text))" }
        line += " \(node.role)"
        if let tooltip = node.tooltip { line += " help=\(quoted(tooltip))" }
        if let value = node.value { line += " value=\(quoted(value))" }
        for (key, value) in node.state.sorted(by: { $0.key < $1.key }) { line += " \(key)=\(value)" }
        if !node.showing { line += " (hidden)" }
        if !node.enabled { line += " (disabled)" }
        return line
    }

    /// A text cut to one line of a reasonable length, for a dump read by eye or by a model.
    private static func quoted(_ text: String) -> String {
        let flat = text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        let cut = flat.count > 120 ? String(flat.prefix(119)) + "…" : flat
        return "\"\(cut.replacingOccurrences(of: "\"", with: "\\\""))\""
    }

    static func dump(_ args: inout AgentArgs) throws -> [String: Any] {
        let under = try args.count("under")
        let window = try args.string("window")
        let hidden = try args.bool("hidden") ?? false
        let full = try args.bool("full") ?? false
        try args.finish()
        let roots = try under.map { [try element($0)] } ?? windows(titled: window)
        var text = ""
        for root in roots {
            var path: [NSObject] = []
            write(root, depth: 0, hidden: hidden, full: full, above: [], path: &path, into: &text)
        }
        return ["text": text]
    }

    /// The windows whose title contains `title`, or every window.
    static func windows(titled title: String?) -> [NSObject] {
        AgentPlatform.windows().filter { window in
            guard let title else { return true }
            return (AgentPlatform.windowFacts(window)["title"] as? String)?.contains(title) == true
        }
    }

    /// The roles that carry nothing of their own: a compact dump folds them into their children.
    private static let plainRoles: Set<String> = [
        "AXGroup", "AXUnknown", "AXSplitGroup", "AXScrollArea", "AXLayoutArea", "AXElement",
    ]

    private static func write(
        _ object: NSObject, depth: Int, hidden: Bool, full: Bool, above: [String],
        path: inout [NSObject], into text: inout String
    ) {
        // The same guard as `walk`: an accessibility tree is the app's own claim about itself.
        guard path.count < 128, !path.contains(where: { $0 === object }) else { return }
        path.append(object)
        defer { path.removeLast() }
        let node = AgentNode(object)
        // An element off screen is left out, but not what it holds: a container can measure nothing
        // while its children are drawn.
        let visible = hidden || node.showing
        let carries = !node.texts.isEmpty || node.identifier != nil || node.tooltip != nil
            || node.value != nil || !plainRoles.contains(node.role)
        // A static text or image that only repeats the line above it is that line's own label.
        let echoes = ["AXStaticText", "AXImage"].contains(node.role) && node.identifier == nil
            && node.texts.allSatisfy(above.contains)
        let shown = visible && (full || (carries && !echoes))
        if shown {
            text += String(repeating: "  ", count: depth) + summary(node) + "\n"
        }
        for child in children(of: object) {
            write(
                child, depth: shown ? depth + 1 : depth, hidden: hidden, full: full,
                above: shown ? node.texts : above, path: &path, into: &text)
        }
    }
}
#endif
