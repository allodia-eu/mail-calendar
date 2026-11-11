// The test agent (MailcalUI/TestAgent) against real windows: its wire, its refusals, and on macOS
// the accessibility it reads, AppKit's controls and a SwiftUI view's alike. The protocol is
// docs/test-agent.md; scripts/dev/agent_conformance.py holds a running app to it.

#if DEBUG
import Foundation
import Testing

@testable import MailcalUI

#if os(macOS)
import AppKit
import SwiftUI
#endif

@MainActor
@Suite(.serialized)
struct TestAgentTests {
    private func ask(_ request: [String: Any]) async -> [String: Any] {
        let line = String(decoding: try! JSONSerialization.data(withJSONObject: request), as: UTF8.self)
        let reply = await AgentProtocol.handle(line)
        return try! JSONSerialization.jsonObject(with: Data(reply.utf8)) as! [String: Any]
    }

    private func code(_ request: [String: Any]) async -> String? {
        let reply = await ask(request)
        #expect(reply["ok"] as? Bool == false, "\(request) answered \(reply)")
        return reply["code"] as? String
    }

    @Test func aRequestWithoutAVerbOrWithAnUnknownOneIsRefusedByItsCode() async {
        #expect(await code(["text": "Sync"]) == "bad-request")
        #expect(await code(["verb": "fly"]) == "unknown-verb")
        let reply = await AgentProtocol.handle("not json")
        #expect(reply.contains(#""code":"bad-request""#))
    }

    @Test func aMisspeltOrMistypedArgumentIsRefusedRatherThanIgnored() async {
        #expect(await code(["verb": "query", "txt": "Sync"]) == "bad-request")
        #expect(await code(["verb": "ping", "text": "Sync"]) == "bad-request")
        #expect(await code(["verb": "wait", "text": "Sync", "timeout": 5]) == "bad-request")
        #expect(await code(["verb": "read", "h": "1"]) == "bad-request")
        #expect(await code(["verb": "query", "enabled": 1]) == "bad-request")
        #expect(await code(["verb": "scroll", "h": 1, "direction": "sideways"]) == "bad-request")
    }

    @Test func everyListedVerbIsRoutedAndDescribed() async {
        let listed = (await ask(["verb": "verbs"]))["verbs"] as? [[String: String]] ?? []
        let required: Set = [
            "verbs", "ping", "windows", "query", "wait", "read", "dump", "activate", "set-text",
            "toggle", "select", "focus", "settle", "screenshot", "quit",
        ]
        #expect(required.isSubset(of: Set(listed.compactMap { $0["verb"] })))
        for entry in listed where entry["verb"] != "quit" {
            #expect(entry["about"]?.isEmpty == false)
            // A bare `wait` matches nothing until its timeout: a short one keeps this quick.
            let reply = await ask(
                ["verb": entry["verb"]!].merging(entry["verb"] == "wait" ? ["timeoutMs": 1] : [:]) { $1 })
            #expect(reply["code"] as? String != "unknown-verb", "\(entry) is listed but not routed")
        }
    }

    @Test func aHandleNeverHandedOutIsRefused() async {
        #expect(await code(["verb": "read", "h": Int(Int32.max)]) == "no-element")
        #expect(await code(["verb": "activate", "h": Int(Int32.max)]) == "no-element")
    }

    @Test func theSocketAnswersItsOwnerAlone() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("agent-\(getpid())")
        let path = directory.appendingPathComponent("agent.sock").path
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data("left behind by an earlier run".utf8).write(to: URL(fileURLWithPath: path))
        let socket = try TestAgent.bindSocket(at: path)
        defer {
            close(socket)
            try? FileManager.default.removeItem(at: directory)
        }
        let mode = { (path: String) in
            (try? FileManager.default.attributesOfItem(atPath: path)[.posixPermissions] as? Int) ?? 0
        }
        #expect(mode(path) == 0o600)
        #expect(mode(directory.path) == 0o700)
    }

    #if os(macOS)
    private final class Clicks: NSObject {
        var count = 0
        @objc func clicked(_ sender: Any?) { count += 1 }
    }

    private struct Fixture {
        let window: NSWindow
        let clicks: Clicks
        let field: NSTextField
    }

    /// A window of AppKit controls and a SwiftUI view, on screen and found by its title: one for
    /// the whole suite, which runs its tests one at a time.
    private static var shared: Fixture?

    private static func makeFixture() -> Fixture {
        let clicks = Clicks()
        let archive = NSButton(title: "Archive", target: clicks, action: #selector(Clicks.clicked))
        let locked = NSButton(title: "Delete", target: clicks, action: #selector(Clicks.clicked))
        locked.isEnabled = false
        let field = NSTextField(string: "")
        field.setAccessibilityIdentifier("search")
        let label = NSTextField(labelWithString: "Inbox")
        let swiftUI = NSHostingView(rootView: Button("Sync") { clicks.count += 1 }.padding())
        let stack = NSStackView(views: [label, archive, locked, field, swiftUI])
        stack.orientation = .vertical
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 400, height: 400),
            styleMask: [.titled], backing: .buffered, defer: false)
        window.title = "Agent test"
        window.isReleasedWhenClosed = false
        window.contentView = stack
        window.orderFront(nil)
        window.layoutIfNeeded()
        return Fixture(window: window, clicks: clicks, field: field)
    }

    private func fixture() -> Fixture {
        let fixture = Self.shared ?? Self.makeFixture()
        Self.shared = fixture
        fixture.clicks.count = 0
        return fixture
    }

    private func only(_ filter: [String: Any]) async -> Int? {
        let reply = await ask(["verb": "query", "window": "Agent test"].merging(filter) { $1 })
        let elements = reply["elements"] as? [[String: Any]] ?? []
        #expect(elements.count == 1, "\(filter) matched \(reply)")
        return elements.first?["h"] as? Int
    }

    // An AppKit button is found and refused here but not pressed: its cell clicks on a later turn
    // of the run loop, which a test process without a running application cannot give it. The
    // running app's conformance run presses one.
    @Test func anAppKitControlIsFoundByWhatItSaysAndADisabledOneRefused() async throws {
        let clicks = fixture().clicks
        let archive = try #require(await only(["text": "Archive", "role": "AXButton"]))
        let read = await ask(["verb": "read", "h": archive])
        #expect(read["showing"] as? Bool == true)
        #expect(read["enabled"] as? Bool == true)
        #expect((read["bounds"] as? [Int])?.count == 4)
        let delete = try #require(await only(["text": "Delete"]))
        #expect(await code(["verb": "activate", "h": delete]) == "disabled")
        #expect(clicks.count == 0)
    }

    @Test func textTypedIntoAFieldReadsBackAndALabelTakesNone() async throws {
        let field = fixture().field
        let search = try #require(await only(["id": "search"]))
        #expect((await ask(["verb": "set-text", "h": search, "text": "invoice"]))["ok"] as? Bool == true)
        #expect(field.stringValue == "invoice")
        #expect((await ask(["verb": "read", "h": search]))["value"] as? String == "invoice")
        let inbox = try #require(await only(["text": "Inbox", "role": "AXStaticText"]))
        #expect(await code(["verb": "set-text", "h": inbox, "text": "x"]) == "not-editable")
    }

    @Test func aSwiftUIViewIsReadAsVoiceOverReadsIt() async throws {
        let clicks = fixture().clicks
        let sync = try #require(await only(["text": "Sync", "role": "AXButton"]))
        #expect((await ask(["verb": "activate", "h": sync]))["how"] as? String == "pressed")
        #expect(clicks.count == 1)
    }

    @Test func aWaitThatRunsOutSaysWhatWasNear() async {
        _ = fixture()
        let reply = await ask(["verb": "wait", "text": "Archive all", "timeoutMs": 100])
        #expect(reply["code"] as? String == "timeout")
        #expect("\(reply["near"] ?? "")".contains("Archive"))
        let gone = await ask(["verb": "wait", "text": "Nothing", "absent": true, "timeoutMs": 100])
        #expect(gone["ok"] as? Bool == true)
    }

    @Test func theDumpSaysEachThingOnce() async {
        _ = fixture()
        let text = (await ask(["verb": "dump", "window": "Agent test"]))["text"] as? String ?? ""
        #expect(text.components(separatedBy: "\"Archive\"").count == 2, "\(text)")
        #expect(text.contains("\"Sync\""), "\(text)")
    }

    @Test func aWindowDrawsItsOwnPicture() async throws {
        _ = fixture()
        let path = FileManager.default.temporaryDirectory
            .appendingPathComponent("agent-shot-\(getpid()).png").path
        defer { try? FileManager.default.removeItem(atPath: path) }
        let reply = await ask(["verb": "screenshot", "path": path])
        #expect((reply["width"] as? Int ?? 0) > 0)
        let bytes = try Data(contentsOf: URL(fileURLWithPath: path))
        #expect(bytes.starts(with: [0x89, 0x50, 0x4E, 0x47]))
    }

    @Test func anIdleAppSettles() async {
        _ = fixture()
        let reply = await ask(["verb": "settle", "quietMs": 100, "capMs": 5000])
        #expect(reply["settled"] as? Bool == true, "\(reply)")
    }
    #endif
}
#endif
