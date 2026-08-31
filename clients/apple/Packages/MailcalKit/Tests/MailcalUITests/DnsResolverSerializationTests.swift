// The Apple resolver must serialize its libresolv calls: Apple's res_9_query family crashes when
// several detection lookups hit it at once (a SIGSEGV in dns_res_send/sock_eq), and the detection
// engine fans MX + SRV lookups out on parallel blocking threads. The crash itself needs a device
// and is a race, so it can't be asserted directly here, but the fix (one process-wide lock) can:
// this proves that lock actually grants mutual exclusion, so a change that drops it, or adds a
// second lock, fails here instead of as an on-device segfault at account setup.

import Foundation
import Testing

@testable import MailcalUI

struct DnsResolverSerializationTests {
    @Test func queryLockSerializesConcurrentCallers() {
        let iterations = 1_000
        // Deliberately non-atomic: the lock is the only thing that may keep these consistent. If
        // withQueryLock ever stops serializing, `active` exceeds 1 (and this racy access is itself
        // the corruption the on-device crash comes from).
        var active = 0
        var maxObserved = 0
        let group = DispatchGroup()
        let queue = DispatchQueue(label: "dns.serialization.test", attributes: .concurrent)
        for _ in 0..<iterations {
            queue.async(group: group) {
                SystemMxResolver.withQueryLock {
                    active += 1
                    maxObserved = max(maxObserved, active)
                    active -= 1
                }
            }
        }
        group.wait()
        #expect(maxObserved == 1)
    }
}
