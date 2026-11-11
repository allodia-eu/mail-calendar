// The verbs that act on one element: every argument read and checked first, then the element held
// to what a person could do, and only then the platform's action (TestAgentMacActions,
// TestAgentPhoneActions).

#if DEBUG
import Foundation

@MainActor
enum AgentActions {
    static func route(_ verb: String, _ args: inout AgentArgs) throws -> [String: Any] {
        switch verb {
        case "activate":
            let h = try args.handle()
            try args.finish()
            return try AgentPlatform.activate(try usable(h))
        case "set-text":
            let h = try args.handle()
            guard let text = try args.string("text") else {
                throw AgentFailure.badRequest("set-text needs text")
            }
            let submit = try args.bool("activate") ?? false
            try args.finish()
            try AgentPlatform.setText(try usable(h), to: text, submit: submit)
            return [:]
        case "toggle":
            let h = try args.handle()
            let active = try args.bool("active")
            try args.finish()
            return ["active": try AgentPlatform.toggle(try usable(h), to: active)]
        case "select":
            let h = try args.handle()
            guard let index = try args.count("index") else {
                throw AgentFailure.badRequest("select needs index")
            }
            try args.finish()
            try AgentPlatform.select(try usable(h), index: index)
            return [:]
        case "focus":
            let h = try args.handle()
            try args.finish()
            return ["focused": AgentPlatform.focus(try AgentTree.element(h))]
        case "escape":
            let h = try args.handle()
            try args.finish()
            return ["how": try AgentPlatform.escape(from: try AgentTree.element(h))]
        case "scroll":
            let h = try args.handle()
            guard let direction = try args.string("direction"),
                  ["up", "down", "left", "right"].contains(direction)
            else { throw AgentFailure.badRequest("direction is up, down, left or right") }
            try args.finish()
            try AgentPlatform.scroll(from: try AgentTree.element(h), direction: direction)
            return [:]
        case "increment", "decrement":
            let h = try args.handle()
            try args.finish()
            let value = try AgentPlatform.step(try usable(h), up: verb == "increment")
            return ["value": value ?? NSNull()]
        default:
            throw AgentFailure(#"no verb "\#(verb)": {"verb": "verbs"} lists them"#, "unknown-verb")
        }
    }

    /// The element a handle names, refused when a person could not act on it either.
    private static func usable(_ h: Int) throws -> NSObject {
        let object = try AgentTree.element(h)
        let node = AgentNode(object)
        if !node.enabled {
            throw AgentFailure("the element is disabled", "disabled")
                .with("element", AgentTree.describe(node))
        }
        if !node.showing {
            throw AgentFailure("the element is not on screen", "not-showing")
                .with("element", AgentTree.describe(node))
        }
        return object
    }
}
#endif
