// Waiting: for an element to appear or go, and for the app to go quiet.
//
// Both poll on the main actor and sleep between polls, so a wait never blocks the app it is
// watching.

#if DEBUG
import Foundation
import Synchronization

@MainActor
enum AgentWait {
    /// How often a wait looks again: about three frames.
    static let poll = Duration.milliseconds(50)

    static func wait(_ args: inout AgentArgs) async throws -> [String: Any] {
        let absent = try args.bool("absent") ?? false
        let timeoutMs = try args.count("timeoutMs") ?? 10_000
        let filter = try AgentFilter(&args)
        try args.finish()
        let clock = ContinuousClock()
        let started = clock.now
        while true {
            await AgentPlatform.awaken()
            let found = try filter.find()
            let ms = milliseconds(since: started)
            if found.isEmpty == absent {
                return ["elements": found.map(AgentTree.describe), "ms": ms]
            }
            if ms >= timeoutMs {
                if absent {
                    throw AgentFailure("still there after \(timeoutMs) ms", "timeout")
                        .with("elements", found.map { AgentTree.summary(AgentNode($0)) })
                }
                throw AgentFailure("nothing matched within \(timeoutMs) ms", "timeout")
                    .with("near", filter.near())
            }
            try await Task.sleep(for: poll)
        }
    }

    /// Waits until neither the accessibility tree nor the core has changed for `quietMs`. Answers
    /// `settled: false` at the cap rather than failing: an app with a spinner never settles, and the
    /// caller decides whether that matters.
    static func settle(_ args: inout AgentArgs) async throws -> [String: Any] {
        let quietMs = try args.count("quietMs") ?? 300
        let capMs = try args.count("capMs") ?? 10_000
        try args.finish()
        let clock = ContinuousClock()
        let started = clock.now
        var last = fingerprint()
        var stillSince = clock.now
        while true {
            try await Task.sleep(for: poll)
            await AgentPlatform.awaken()
            let now = fingerprint()
            if now != last {
                last = now
                stillSince = clock.now
            } else if milliseconds(since: stillSince) >= quietMs {
                return ["ms": milliseconds(since: started), "settled": true]
            }
            if milliseconds(since: started) >= capMs {
                return ["ms": milliseconds(since: started), "settled": false]
            }
        }
    }

    /// What is on screen and how far the core has got: every showing element's line, without its
    /// handle, and the count of change signals.
    static func fingerprint() -> Int {
        var hasher = Hasher()
        hasher.combine(TestAgent.surfaces.load(ordering: .relaxed))
        for window in AgentPlatform.windows() {
            AgentTree.walk(window) { object, _ in
                let node = AgentNode(object)
                if node.showing { hasher.combine(AgentTree.summary(node, handle: false)) }
                return true
            }
        }
        return hasher.finalize()
    }

    private static func milliseconds(since start: ContinuousClock.Instant) -> Int {
        let elapsed = ContinuousClock.now - start
        return Int(elapsed.components.seconds * 1000)
            + Int(elapsed.components.attoseconds / 1_000_000_000_000_000)
    }
}
#endif
