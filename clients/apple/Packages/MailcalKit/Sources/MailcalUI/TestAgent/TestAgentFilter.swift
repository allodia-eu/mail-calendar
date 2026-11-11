// Finding elements: the filter every lookup takes, and what a lookup that found nothing offers
// instead.

#if DEBUG
import Foundation

@MainActor
struct AgentFilter {
    /// A label, title, help text or value, whole.
    var text: String?
    /// Part of one, in any case.
    var textLike: String?
    /// A class name; subclasses match too.
    var type: String?
    /// The accessibilityIdentifier a developer gave it, which no person reads.
    var id: String?
    /// Accessibility names no icon, so a filter on one matches nothing here.
    var icon: String?
    var role: String?
    /// Part of a window's title.
    var window: String?
    var under: Int?
    var hidden = false
    /// Whether a person could act on it.
    var enabled: Bool?
    /// A filter some descendant has to match: at most one.
    var has: [AgentFilter] = []
    /// Only the match at this position, counting from 0.
    var index: Int?

    /// The most a lookup returns, so a filter that matches every label cannot flood the caller.
    static let most = 200

    init() {}

    init(_ args: inout AgentArgs) throws {
        text = try args.string("text")
        textLike = try args.string("textLike")
        type = try args.string("type")
        id = try args.string("id")
        icon = try args.string("icon")
        role = try args.string("role")
        window = try args.string("window")
        under = try args.count("under")
        hidden = try args.bool("hidden") ?? false
        enabled = try args.bool("enabled")
        index = try args.count("index")
        if let nested = try args.object("has") {
            var inner = AgentArgs(nested)
            has = [try AgentFilter(&inner)]
            try inner.finish()
        }
    }

    func matches(_ node: AgentNode) -> Bool {
        if !hidden && !node.showing { return false }
        if let enabled, node.enabled != enabled { return false }
        if let type, !node.isKind(of: type) { return false }
        if let id, node.identifier != id { return false }
        if icon != nil { return false }
        if let role, node.role != role { return false }
        if text != nil || textLike != nil {
            let said = node.said
            if let text, !said.contains(text) { return false }
            if let like = textLike?.lowercased(),
               !said.contains(where: { $0.lowercased().contains(like) }) {
                return false
            }
        }
        if let has = has.first {
            var found = false
            for child in AgentTree.children(of: node.object) {
                AgentTree.walk(child) { object, _ in
                    found = has.matches(AgentNode(object))
                    return !found
                }
                if found { break }
            }
            if !found { return false }
        }
        return true
    }

    func roots() throws -> [NSObject] {
        if let under { return [try AgentTree.element(under)] }
        return AgentTree.windows(titled: window)
    }

    func find() throws -> [NSObject] {
        var found: [(node: AgentNode, ancestors: [NSObject])] = []
        for root in try roots() {
            AgentTree.walk(root) { object, ancestors in
                let node = AgentNode(object)
                if matches(node) { found.append((node, ancestors)) }
                return found.count < Self.most
            }
        }
        let kept = Self.withoutEchoes(found)
        if let index {
            return kept.indices.contains(index) ? [kept[index]] : []
        }
        return kept
    }

    /// Leaves out a match that only repeats an enclosing match: the text inside a button says the
    /// button's words, and the button is the thing a caller means.
    private static func withoutEchoes(
        _ found: [(node: AgentNode, ancestors: [NSObject])]
    ) -> [NSObject] {
        found.filter { inner in
            let says = inner.node.said
            return says.isEmpty || !found.contains { outer in
                outer.node !== inner.node
                    && inner.ancestors.contains { $0 === outer.node.object }
                    && says.allSatisfy(outer.node.said.contains)
            }
        }.map(\.node.object)
    }

    /// What a lookup that found nothing might have meant: elements on screen whose text shares a
    /// word with the one asked for. The usual miss is a label that changed, and this names the new
    /// one.
    func near() -> [String] {
        guard let wanted = text ?? textLike else { return [] }
        let words = wanted.lowercased().split(whereSeparator: \.isWhitespace)
            .filter { $0.count > 2 }.map(String.init)
        var near: [String] = []
        guard !words.isEmpty, let roots = try? roots() else { return [] }
        for root in roots {
            AgentTree.walk(root) { object, _ in
                let node = AgentNode(object)
                if node.showing, node.said.contains(where: { said in
                    words.contains { said.lowercased().contains($0) }
                }) {
                    near.append(AgentTree.summary(node))
                }
                return near.count < 8
            }
        }
        return near
    }
}
#endif
