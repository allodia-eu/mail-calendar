// A background pass waits for a foreground core that is still opening.
//
// The foreground core opens off the main actor (docs/boot-sequence.md), so for the length of that
// open `LiveCore` holds no core although the process is about to have one. A pass that read the
// empty handle there would build a second core over the same store: a second refresher on every
// account's grant. The pass itself is iOS-only, and `swift test` runs on macOS, so the handshake it
// waits on is what is tested here.

import Synchronization
import Testing

@testable import MailcalUI

/// Whether the waiting pass has moved on, readable from the test while it waits.
private final class Finished: Sendable {
    private let flag = Mutex(false)
    func set() { flag.withLock { $0 = true } }
    var isSet: Bool { flag.withLock { $0 } }
}

@Suite(.timeLimit(.minutes(1))) struct LiveCoreTests {

    @Test func aPassWithNoOpenInProgressDoesNotWait() async {
        let live = LiveCore()
        await live.waitWhileOpening()
        #expect(!live.isOpening)
    }

    @Test func aPassWaitsUntilTheForegroundCoreHasOpened() async throws {
        let live = LiveCore()
        live.beginOpening()
        let finished = Finished()
        let waiter = Task {
            await live.waitWhileOpening()
            finished.set()
        }
        try await Task.sleep(for: .milliseconds(500))
        #expect(!finished.isSet, "the pass must not decide while the core opens")
        live.set(nil)
        await waiter.value
        #expect(finished.isSet)
    }

    @Test func aFailedOpenReleasesTheWaitingPass() async {
        let live = LiveCore()
        live.beginOpening()
        let waiter = Task { await live.waitWhileOpening() }
        live.openingFailed()
        await waiter.value
        #expect(!live.isOpening)
    }

    @Test func theOSEndingThePassEndsTheWait() async {
        let live = LiveCore()
        live.beginOpening()
        let waiter = Task { await live.waitWhileOpening() }
        waiter.cancel()
        await waiter.value
        #expect(live.isOpening, "cancellation ends the wait, not the open")
    }
}
