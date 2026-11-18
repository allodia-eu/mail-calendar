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
        elements(try agent.call("query", filter.fields), filter)
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
        return try explained(because, filter) { elements(try agent.call("wait", arguments), filter) }
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
    /// Once the screen is still, as a person waits: a menu opened while a sheet is still coming up
    /// is closed again by SwiftUI, and an element found while it slid in is not where it was.
    func activate(_ element: Element) throws {
        try settle(quietMs: Self.beforeActing, capMs: Self.capBeforeActing)
        let element = try current(element)
        do {
            try agent.call("activate", ["h": element.h])
        } catch let refusal as AgentRefusal where refusal.code == "not-activatable" {
            try tap(element)
        }
    }

    /// Opens `menu` and presses its item `item`, as a person does with two taps, opening it again
    /// if it closed before the item was pressed: a SwiftUI menu now and then closes itself just
    /// after opening, on a still screen, and a person would simply open it again.
    func choose(_ item: Filter, from menu: Element, _ because: String? = nil) throws {
        for attempt in 1...3 {
            try activate(menu)
            do {
                try press(item, timeout: 5, because)
                return
            } catch let refusal as AgentRefusal
                where attempt < 3 && ["not-showing", "timeout"].contains(refusal.code) {
                continue
            }
        }
    }

    /// `element`, or what its filter finds in its place once the toolkit has replaced it: SwiftUI
    /// rebuilds a control's accessibility object when its view updates, which a settle can see.
    private func current(_ element: Element) throws -> Element {
        do {
            _ = try read(element)
            return element
        } catch let refusal as AgentRefusal where refusal.code == "gone" {
            guard let found = element.found else { throw refusal }
            return try wait(found, timeout: 5, "it was replaced and is not back")
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
    ///
    /// A screen that never settles costs the whole cap, so one that did not is said in the log,
    /// with what the agent saw still moving.
    func settle(quietMs: Int = 300, capMs: Int = 10_000) throws {
        let reply = try agent.call("settle", ["quietMs": quietMs, "capMs": capMs])
        if reply["settled"] as? Bool == false {
            let moving = reply["moving"] as? String ?? "the tree kept changing"
            print("settle gave up after \(reply["ms"] ?? capMs) ms: \(moving)")
        }
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
    func tap(_ element: Element, at offset: CGVector = CGVector(dx: 0.5, dy: 0.5)) throws {
        try settle(quietMs: Self.beforeActing, capMs: Self.capBeforeActing)
        try point(current(element), at: offset).tap()
    }

    func longPress(_ element: Element, duration: TimeInterval = 1.2) throws {
        try settle(quietMs: Self.beforeActing, capMs: Self.capBeforeActing)
        try point(current(element)).press(forDuration: duration)
    }

    /// How long the screen has to have been still before an action. Short, because `settle` also
    /// waits out anything still moving; a read that follows a change asks for the full default.
    private static let beforeActing = 100
    /// How long an action waits for a screen that keeps moving before it acts anyway.
    private static let capBeforeActing = 3_000

    /// Sends a chord from the keyboard, and returns whether `arrived` came to hold.
    ///
    /// ⚠️ On an iPad simulator a chord into a focused field is now and then dropped before the app
    /// sees it: nothing changes, and the same chord sent a second later arrives. So a chord whose
    /// effect has not shown within a second is sent once more. A chord the app ignores is ignored
    /// both times, and the test still fails.
    func chord(
        _ key: String, _ modifiers: XCUIElement.KeyModifierFlags, timeout: TimeInterval = 5,
        arrived: () throws -> Bool
    ) throws -> Bool {
        input.typeKey(key, modifierFlags: modifiers)
        if try holds(within: 1, arrived) { return true }
        input.typeKey(key, modifierFlags: modifiers)
        return try holds(within: timeout, arrived)
    }

    // MARK: Helpers

    private func holds(within timeout: TimeInterval, _ condition: () throws -> Bool) throws -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while true {
            if try condition() { return true }
            if Date() >= deadline { return false }
            Thread.sleep(forTimeInterval: 0.1)
        }
    }

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

    private func elements(_ reply: [String: Any], _ filter: Filter) -> [Element] {
        (reply["elements"] as? [[String: Any]] ?? []).enumerated().map { index, fields in
            var element = Element(fields)
            // A filter that already names a position keeps it: its one match is that position.
            element.found = filter.fields["index"] == nil ? filter.index(index) : filter
            return element
        }
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
