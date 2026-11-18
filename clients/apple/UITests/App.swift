// The app under test, as a test drives it: found, read and acted on through its test agent
// (docs/test-agent.md), touched through XCUITest only where real input is the thing under test.
//
// The agent reads the app's own accessibility objects in process, so a lookup costs milliseconds
// where an XCUITest query snapshots the whole tree across processes; and it acts as VoiceOver does,
// so it refuses what a person could not do. What it cannot give is a finger or a keyboard: a long
// press, a pinch, a drag, a tap whose landing place is the question, typed keys and chords. Those
// go through `input`, aimed at the frame the agent reported.

import XCTest

@MainActor
final class App {
    /// How long to wait for a surface that has to load mail before it can draw.
    static let timeout: TimeInterval = 30

    /// The real input a person gives: keys, typing, touches and gestures.
    let input: XCUIApplication
    let agent: Agent

    init(input: XCUIApplication, agent: Agent) {
        self.input = input
        self.agent = agent
    }

    // MARK: Finding

    /// What matches now, without waiting.
    func query(_ filter: Filter) throws -> [Element] {
        elements(try agent.call("query", filter.fields))
    }

    func exists(_ filter: Filter) throws -> Bool {
        try !query(filter).isEmpty
    }

    /// The first element `filter` matches, once one does.
    @discardableResult
    func wait(
        _ filter: Filter, timeout: TimeInterval = App.timeout, _ because: String? = nil
    ) throws -> Element {
        try waitAll(filter, timeout: timeout, because)[0]
    }

    /// Everything `filter` matches, once anything does.
    func waitAll(
        _ filter: Filter, timeout: TimeInterval = App.timeout, _ because: String? = nil
    ) throws -> [Element] {
        var arguments = filter.fields
        arguments["timeoutMs"] = Int(timeout * 1000)
        return try explained(because, filter) { elements(try agent.call("wait", arguments)) }
    }

    /// Returns once nothing matches `filter`.
    func waitGone(
        _ filter: Filter, timeout: TimeInterval = 5, _ because: String? = nil
    ) throws {
        var arguments = filter.fields
        arguments["absent"] = true
        arguments["timeoutMs"] = Int(timeout * 1000)
        try explained(because, filter) { _ = try agent.call("wait", arguments) }
    }

    /// Whether something matches `filter` within `timeout`: for an assertion with its own message.
    func appears(_ filter: Filter, timeout: TimeInterval = App.timeout) throws -> Bool {
        try timedOut { try wait(filter, timeout: timeout) }
    }

    /// Whether nothing matches `filter` within `timeout`.
    func goes(_ filter: Filter, timeout: TimeInterval = 5) throws -> Bool {
        try timedOut { try waitGone(filter, timeout: timeout) }
    }

    /// The element as it is now.
    func read(_ element: Element) throws -> Element {
        Element(try agent.call("read", ["h": element.h]))
    }

    // MARK: Acting, as VoiceOver does

    /// Waits for exactly one element `filter` matches, and activates it.
    @discardableResult
    func press(
        _ filter: Filter, timeout: TimeInterval = App.timeout, _ because: String? = nil
    ) throws -> Element {
        let element = try only(filter, timeout: timeout, because)
        try explained(because, filter) { try activate(element) }
        return element
    }

    /// What VoiceOver's double tap does: the element's accessibility activation, or, for one that
    /// takes none (a list row, a menu button, UIKit's back button), a tap where it is drawn.
    ///
    /// An element found while it was still sliding in (a menu's item, a sheet's button) can be
    /// refused as not yet on screen: it is asked again once the screen is still.
    func activate(_ element: Element) throws {
        do {
            try activateOnce(element)
        } catch let refusal as AgentRefusal where refusal.code == "not-showing" {
            try settle()
            try activateOnce(element)
        }
    }

    private func activateOnce(_ element: Element) throws {
        do {
            try agent.call("activate", ["h": element.h])
        } catch let refusal as AgentRefusal where refusal.code == "not-activatable" {
            try tap(element)
        }
    }

    /// Waits for exactly one field `filter` matches, and replaces its text as typing would.
    func fill(_ filter: Filter, _ text: String, submit: Bool = false) throws {
        let field = try only(filter, timeout: App.timeout, nil)
        try agent.call("set-text", ["h": field.h, "text": text, "activate": submit])
    }

    @discardableResult
    func toggle(_ element: Element, to active: Bool? = nil) throws -> Bool {
        var arguments: [String: Any] = ["h": element.h]
        if let active { arguments["active"] = active }
        return try agent.call("toggle", arguments)["active"] as? Bool ?? false
    }

    func select(_ element: Element, index: Int) throws {
        try agent.call("select", ["h": element.h, "index": index])
    }

    /// VoiceOver's escape from `element` outwards: ends a search, closes a sheet or a menu.
    func escape(from element: Element) throws {
        try agent.call("escape", ["h": element.h])
    }

    /// VoiceOver's three-finger scroll by a page; `up` shows what is above.
    func scroll(_ element: Element, _ direction: String) throws {
        try agent.call("scroll", ["h": element.h, "direction": direction])
    }

    /// Returns once neither the screen nor the core has changed for `quietMs`.
    func settle(quietMs: Int = 300) throws {
        try agent.call("settle", ["quietMs": quietMs])
    }

    /// The tree as the agent reads it, one element a line: for a failure message.
    func dump() -> String {
        (try? agent.call("dump"))?["text"] as? String ?? "(no dump)"
    }

    // MARK: Touching, where the touch is the question

    /// The point at the element's centre, for a gesture.
    func point(_ element: Element, at offset: CGVector = CGVector(dx: 0.5, dy: 0.5)) throws
        -> XCUICoordinate {
        guard let frame = try read(element).frame, frame.width > 0, frame.height > 0 else {
            throw AgentRefusal(
                verb: "point", code: "not-showing", reply: ["error": "\(element) has no frame"])
        }
        return input.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(
            dx: frame.minX + frame.width * offset.dx, dy: frame.minY + frame.height * offset.dy))
    }

    /// A finger on the element, once the screen has stopped moving: a frame read mid-animation aims
    /// the touch where the element was.
    func tap(_ element: Element) throws {
        try settle()
        try point(element).tap()
    }

    func longPress(_ element: Element, duration: TimeInterval = 1.2) throws {
        try settle()
        try point(element).press(forDuration: duration)
    }

    // MARK: Helpers

    /// The one element `filter` matches. Two are a refusal, as `press` refuses two
    /// (docs/test-agent.md): a test that cannot say which one it means has found a different screen.
    private func only(
        _ filter: Filter, timeout: TimeInterval, _ because: String?
    ) throws -> Element {
        let found = try waitAll(filter, timeout: timeout, because)
        guard found.count == 1 else {
            throw AgentRefusal(
                verb: "press", code: "ambiguous",
                reply: ["error": "\(found.count) match \(filter)", "elements": found.map(\.description)],
                because: because)
        }
        return found[0]
    }

    private func elements(_ reply: [String: Any]) -> [Element] {
        (reply["elements"] as? [[String: Any]] ?? []).map(Element.init)
    }

    /// Puts the test's own words, and the filter it was looking for, in front of a refusal.
    private func explained<T>(
        _ because: String?, _ filter: Filter, _ body: () throws -> T
    ) throws -> T {
        do {
            return try body()
        } catch var refusal as AgentRefusal {
            let looking = "looking for \(filter)"
            refusal.because = because.map { "\($0) (\(looking))" } ?? looking
            throw refusal
        }
    }

    private func timedOut(_ body: () throws -> Void) throws -> Bool {
        do {
            try body()
            return true
        } catch let refusal as AgentRefusal where refusal.code == "timeout" {
            return false
        }
    }
}
