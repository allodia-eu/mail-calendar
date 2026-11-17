// The app's test agent (docs/test-agent.md), spoken from the test runner.
//
// The runner and the app it launches both run on the Mac's filesystem, which the simulator
// shares, so the runner reaches the socket the app listens on. A physical device has no such path.

import Foundation
import XCTest

/// A request the agent refused. `code` is what a caller branches on (docs/test-agent.md,
/// "Refusals"); the description carries what the agent said was near.
struct AgentRefusal: Error, CustomStringConvertible {
    let verb: String
    let code: String
    let reply: [String: Any]
    var because: String?

    var description: String {
        var text = "\(verb) refused (\(code)): \(reply["error"] as? String ?? "no reason given")"
        if let because { text = "\(because): \(text)" }
        for key in ["near", "elements"] {
            if let lines = reply[key] as? [String], !lines.isEmpty {
                text += "\n\(key):\n  " + lines.prefix(12).joined(separator: "\n  ")
            }
        }
        return text
    }
}

/// What a lookup matches. Every field narrows it (docs/test-agent.md, "Filters").
struct Filter: CustomStringConvertible {
    fileprivate(set) var fields: [String: Any] = [:]

    /// One of its texts, exactly.
    static func text(_ text: String) -> Filter { Filter().with("text", text) }
    /// Part of one of its texts, in any case.
    static func textLike(_ text: String) -> Filter { Filter().with("textLike", text) }
    /// Its accessibilityIdentifier.
    static func id(_ id: String) -> Filter { Filter().with("id", id) }
    /// Its role, as the agent spells it (`AXButton`, `AXStaticText`, `AXTextField`).
    static func role(_ role: String) -> Filter { Filter().with("role", role) }
    /// A button whose label is `label`.
    static func button(_ label: String) -> Filter { .text(label).role("AXButton") }

    func role(_ role: String) -> Filter { with("role", role) }
    func type(_ type: String) -> Filter { with("type", type) }
    /// Inside the element with handle `h`.
    func under(_ element: Element) -> Filter { with("under", element.h) }
    /// One of its descendants matches `filter`.
    func has(_ filter: Filter) -> Filter { with("has", filter.fields) }
    /// Including elements that are not on screen.
    func hidden() -> Filter { with("hidden", true) }
    func enabled(_ enabled: Bool) -> Filter { with("enabled", enabled) }
    /// Only the match at this position, from 0.
    func index(_ index: Int) -> Filter { with("index", index) }

    private func with(_ key: String, _ value: Any) -> Filter {
        var filter = self
        filter.fields[key] = value
        return filter
    }

    var description: String {
        fields.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" }.joined(separator: " ")
    }
}

/// One element as the agent described it (docs/test-agent.md, "Elements").
struct Element: CustomStringConvertible {
    let h: Int
    let type: String
    let role: String
    let text: [String]
    let id: String?
    let value: String?
    /// In points of its window, which on a phone or a full-screen iPad is the screen.
    let frame: CGRect?
    let showing: Bool
    let enabled: Bool
    let focused: Bool
    let selected: Bool
    let active: Bool?

    init(_ reply: [String: Any]) {
        h = reply["h"] as? Int ?? 0
        type = reply["type"] as? String ?? ""
        role = reply["role"] as? String ?? ""
        text = reply["text"] as? [String] ?? []
        id = reply["id"] as? String
        value = reply["value"] as? String
        if let b = reply["bounds"] as? [Int], b.count == 4 {
            frame = CGRect(x: b[0], y: b[1], width: b[2], height: b[3])
        } else {
            frame = nil
        }
        showing = reply["showing"] as? Bool ?? false
        enabled = reply["enabled"] as? Bool ?? false
        focused = reply["focused"] as? Bool ?? false
        selected = reply["selected"] as? Bool ?? false
        active = reply["active"] as? Bool
    }

    /// What a reader hears first: its first text, or nothing.
    var label: String { text.first ?? "" }

    var description: String {
        "[\(h)] \(type) \(role) \(text.map { "\"\($0)\"" }.joined(separator: " "))"
    }
}

/// One connection to the agent: a JSON object a line each way, one reply per request.
final class Agent {
    let path: String
    private var fd: Int32 = -1
    private var pending = [UInt8]()

    /// Where this simulator's app listens: under the Mac user's own caches, which nobody else can
    /// enter, one socket per simulator. The simulator's own temporary directory is too long a path
    /// for a socket's 104 bytes.
    static var socketPath: String {
        let environment = ProcessInfo.processInfo.environment
        let home = environment["SIMULATOR_HOST_HOME"] ?? NSHomeDirectory()
        let simulator = (environment["SIMULATOR_UDID"] ?? "simulator").prefix(8)
        return "\(home)/Library/Caches/mailcal-agent/uitests-\(simulator).sock"
    }

    init(path: String) {
        self.path = path
    }

    deinit {
        if fd >= 0 { close(fd) }
    }

    /// Connects once the app is listening, which it starts doing as it launches.
    func connect(timeout: TimeInterval) throws {
        let deadline = Date().addingTimeInterval(timeout)
        var last = ""
        while Date() < deadline {
            do {
                fd = try Self.open(path)
                pending = []
                return
            } catch {
                last = "\(error)"
            }
            Thread.sleep(forTimeInterval: 0.05)
        }
        throw AgentRefusal(
            verb: "connect", code: "no-app",
            reply: ["error": "nothing answered on \(path) within \(Int(timeout)) s (\(last))"])
    }

    /// Sends one request and returns its reply, or throws the agent's refusal.
    @discardableResult
    func call(_ verb: String, _ arguments: [String: Any] = [:]) throws -> [String: Any] {
        var request = arguments
        request["verb"] = verb
        let line = try JSONSerialization.data(withJSONObject: request) + Data("\n".utf8)
        try send(line, verb: verb)
        let reply = try receive(verb: verb)
        guard reply["ok"] as? Bool == true else {
            throw AgentRefusal(verb: verb, code: reply["code"] as? String ?? "?", reply: reply)
        }
        return reply
    }

    private func send(_ data: Data, verb: String) throws {
        var bytes = [UInt8](data)[...]
        while !bytes.isEmpty {
            let written = bytes.withUnsafeBytes { write(fd, $0.baseAddress, $0.count) }
            if written < 0 && errno == EINTR { continue }
            guard written > 0 else { throw hungUp(verb) }
            bytes = bytes.dropFirst(written)
        }
    }

    private func receive(verb: String) throws -> [String: Any] {
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        while true {
            if let newline = pending.firstIndex(of: UInt8(ascii: "\n")) {
                let line = Data(pending[..<newline])
                pending.removeSubrange(...newline)
                guard let reply = try JSONSerialization.jsonObject(with: line) as? [String: Any]
                else { throw hungUp(verb) }
                return reply
            }
            let count = read(fd, &buffer, buffer.count)
            if count < 0 && errno == EINTR { continue }
            guard count > 0 else { throw hungUp(verb) }
            pending.append(contentsOf: buffer[0..<count])
        }
    }

    private func hungUp(_ verb: String) -> AgentRefusal {
        AgentRefusal(verb: verb, code: "hung-up", reply: ["error": "the app went before it answered"])
    }

    private static func open(_ path: String) throws -> Int32 {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        let bytes = Array(path.utf8)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: bytes) }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw POSIXError(.EBADF) }
        var on: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        let connected = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else {
            let error = POSIXError(POSIXErrorCode(rawValue: errno) ?? .ECONNREFUSED)
            close(fd)
            throw error
        }
        return fd
    }
}
