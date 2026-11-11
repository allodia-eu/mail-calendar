// Reading a request, routing it to its verb, and the shape of every reply (docs/test-agent.md).

#if DEBUG
import Foundation
import Synchronization

/// A refusal. The code is what a caller branches on; the message is for a person to read.
struct AgentFailure: Error, @unchecked Sendable {
    let message: String
    let code: String
    var extra: [String: Any] = [:]

    init(_ message: String, _ code: String) {
        self.message = message
        self.code = code
    }

    func with(_ key: String, _ value: Any) -> AgentFailure {
        var failure = self
        failure.extra[key] = value
        return failure
    }

    static func badRequest(_ message: String) -> AgentFailure {
        AgentFailure(message, "bad-request")
    }
}

/// A request's arguments, taken one at a time. Whatever is left once a verb has taken what it knows
/// was misspelt, and is refused: never a filter that silently matches everything.
struct AgentArgs {
    private var fields: [String: Any]

    init(_ fields: [String: Any]) {
        self.fields = fields
    }

    mutating func string(_ key: String) throws -> String? {
        guard let value = fields.removeValue(forKey: key) else { return nil }
        guard let text = value as? String else { throw AgentFailure.badRequest("\(key) is text") }
        return text
    }

    mutating func bool(_ key: String) throws -> Bool? {
        guard let value = fields.removeValue(forKey: key) else { return nil }
        guard let number = value as? NSNumber, CFGetTypeID(number) == CFBooleanGetTypeID() else {
            throw AgentFailure.badRequest("\(key) is true or false")
        }
        return number.boolValue
    }

    /// A whole number, never negative.
    mutating func count(_ key: String) throws -> Int? {
        guard let value = fields.removeValue(forKey: key) else { return nil }
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
              number.doubleValue >= 0, number.doubleValue == number.doubleValue.rounded(),
              number.doubleValue < Double(Int.max)
        else { throw AgentFailure.badRequest("\(key) is a whole number") }
        return number.intValue
    }

    mutating func object(_ key: String) throws -> [String: Any]? {
        guard let value = fields.removeValue(forKey: key) else { return nil }
        guard let object = value as? [String: Any] else {
            throw AgentFailure.badRequest("\(key) is an object")
        }
        return object
    }

    /// The handle every verb that acts on one element takes.
    mutating func handle() throws -> Int {
        guard let h = try count("h") else { throw AgentFailure.badRequest("this verb needs h") }
        return h
    }

    func finish() throws {
        if let key = fields.keys.sorted().first {
            throw AgentFailure.badRequest("bad arguments: this verb does not take \(key)")
        }
    }
}

@MainActor
enum AgentProtocol {
    /// Every verb, its arguments and what it answers: the reply to `verbs`, and the protocol's own
    /// documentation. A filter is the argument set `query` takes.
    static let verbs: [(verb: String, about: String)] = [
        ("verbs", "{} -> every verb and its arguments"),
        ("ping", "{} -> {pid, surfaces}: surfaces counts the core's change signals so far"),
        ("windows", "{} -> {windows: [{h, type, title, width, height, active, modal}]}"),
        ("query",
         "{text?, textLike?, type?, id?, icon?, role?, window?, under?, hidden?, enabled?, has?, "
            + "index?} -> {elements: [node]}. text matches a label, title, help or value exactly; "
            + "textLike is a case-insensitive substring; id is the accessibilityIdentifier; type is "
            + "a class name and matches subclasses; role is the accessibility role as the platform "
            + "spells it (AXButton, AXTextField); window is part of a window title; has is a "
            + "filter a descendant must match; hidden includes elements that are not on screen. "
            + "Accessibility names no icon, so icon matches nothing here."),
        ("wait",
         "{filter…, absent?, timeoutMs=10000} -> {elements, ms}. Polls until the filter matches "
            + "(or matches nothing, with absent)."),
        ("read", "{h} -> node"),
        ("dump",
         "{under?, window?, hidden?, full?} -> {text}: the tree, indented. Compact unless full: "
            + "a container that carries nothing is folded into its children."),
        ("activate",
         "{h} -> {how}: the accessibility press VoiceOver performs, done at once; a row that has "
            + "no press is chosen instead."),
        ("set-text",
         "{h, text, activate?} -> {}: replaces a text field's contents as typing would; activate "
            + "then submits, as Return would"),
        ("toggle", "{h, active?} -> {active}: a check box or switch"),
        ("select", "{h, index} -> {}: a pop-up menu, segmented control, tab bar or list"),
        ("focus", "{h} -> {focused}"),
        ("escape",
         "{h} -> {how}: VoiceOver's escape, from h outwards: dismisses the search, sheet or menu "
            + "h is in, as Escape or the two-finger scrub does"),
        ("scroll",
         "{h, direction} -> {}: VoiceOver's three-finger scroll by a page, taken by h or the "
            + "nearest scroll area around it. up shows what is above, as a swipe down does."),
        ("increment", "{h} -> {value}: the accessibility increment of a slider or stepper"),
        ("decrement", "{h} -> {value}: the accessibility decrement of a slider or stepper"),
        ("screenshot",
         "{path, h?} -> {path, width, height}: the window (or h, cut from it) drawn by the app "
            + "into a PNG, at the display's scale"),
        ("settle",
         "{quietMs=300, capMs=10000} -> {ms, settled}: waits until the accessibility tree and "
            + "the core have both been still for quietMs"),
        ("quit", "{} -> {}: quits the app as its own Quit does"),
    ]

    static func handle(_ line: String) async -> String {
        var reply: [String: Any]
        do {
            guard let object = try? JSONSerialization.jsonObject(with: Data(line.utf8)) else {
                throw AgentFailure.badRequest("not JSON")
            }
            guard let request = object as? [String: Any] else {
                throw AgentFailure.badRequest("a request is a JSON object")
            }
            reply = try await route(request)
            reply["ok"] = true
        } catch let failure as AgentFailure {
            reply = failure.extra
            reply["ok"] = false
            reply["error"] = failure.message
            reply["code"] = failure.code
        } catch {
            reply = ["ok": false, "error": "\(error)", "code": "bad-request"]
        }
        return serialised(reply)
    }

    private static func serialised(_ reply: [String: Any]) -> String {
        guard JSONSerialization.isValidJSONObject(reply),
              let data = try? JSONSerialization.data(withJSONObject: reply, options: [.sortedKeys])
        else {
            return #"{"ok":false,"code":"bad-request","error":"the reply could not be written"}"#
        }
        return String(decoding: data, as: UTF8.self)
    }

    static func route(_ request: [String: Any]) async throws -> [String: Any] {
        var fields = request
        guard let verb = fields.removeValue(forKey: "verb") as? String else {
            throw AgentFailure.badRequest(#"a request names its verb: {"verb": "verbs"} lists them"#)
        }
        var args = AgentArgs(fields)
        AgentTree.beginRequest()
        await AgentPlatform.awaken()
        switch verb {
        case "verbs":
            try args.finish()
            return ["verbs": verbs.map { ["verb": $0.verb, "about": $0.about] }]
        case "ping":
            try args.finish()
            return [
                "pid": Int(getpid()),
                "surfaces": Int(TestAgent.surfaces.load(ordering: .relaxed)),
            ]
        case "windows":
            try args.finish()
            return AgentTree.windows()
        case "query":
            let filter = try AgentFilter(&args)
            try args.finish()
            return ["elements": try filter.find().map(AgentTree.describe)]
        case "wait":
            return try await AgentWait.wait(&args)
        case "read":
            let h = try args.handle()
            try args.finish()
            return AgentTree.describe(try AgentTree.element(h))
        case "dump":
            return try AgentTree.dump(&args)
        case "settle":
            return try await AgentWait.settle(&args)
        case "screenshot":
            let path = try args.string("path")
            let h = try args.count("h")
            try args.finish()
            guard let path else { throw AgentFailure.badRequest("screenshot needs path") }
            return try AgentPlatform.screenshot(of: h.map(AgentTree.element), to: path)
        case "quit":
            try args.finish()
            TestAgent.quitAsked.store(true, ordering: .relaxed)
            return [:]
        default:
            return try AgentActions.route(verb, &args)
        }
    }
}
#endif
