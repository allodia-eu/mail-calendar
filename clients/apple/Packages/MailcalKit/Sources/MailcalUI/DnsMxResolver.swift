// The Apple MX/SRV resolver behind the shared detection engine's DNS port. libresolv's
// res_9_query reads the system resolver configuration, sends the query, and returns the raw
// answer, which DnsMessage parses in Swift (reusing the same wire logic the package test suite
// covers). The AD bit is surfaced best-effort, res_9_query doesn't request DNSSEC, but is not
// used in any trust decision today (DNS-derived configs are trusted on CA-validated TLS); it is
// reserved for a future opt-in "require DNSSEC" setting. The core calls this on background
// tasks, so a blocking lookup here is fine.
import CResolv
import Foundation
import MailcalBindings

final class SystemMxResolver: MxResolver, Sendable {
    // Apple's BIND-compatibility resolver is NOT safe to call concurrently. `_res` is per-thread
    // on Darwin (`_res` expands to `*__res_state()`), which isolates the resolver *state struct*:
    // and that alone was mistakenly assumed to make concurrent lookups safe. It does not: res_9_query
    // ultimately runs through `dns_res_send`, whose process-global DNS configuration and socket
    // machinery is shared across threads and unsynchronized. The detection engine fans MX, JMAP-SRV
    // and IMAP/SMTP-SRV lookups out on parallel blocking threads (orchestrator `tokio::spawn` +
    // `spawn_blocking`), so those calls collide inside `dns_res_send` and crash, observed as a
    // SIGSEGV in `sock_eq` while it compares a nameserver address a racing call has torn down.
    // (This stayed hidden until SRV autodiscovery landed a second and third concurrent lookup; the
    // MX-only era only ever ran one at a time.) Serialize every libresolv query behind one
    // process-wide lock. These run only at account setup and return fast (usually NXDOMAIN), so the
    // serialized cost is immaterial.
    private static let queryLock = NSLock()

    /// Runs `body` with every libresolv query in the process serialized (see the `queryLock` note
    /// for why that is required). Exposed for the serialization regression test.
    static func withQueryLock<T>(_ body: () -> T) -> T {
        queryLock.lock()
        defer { queryLock.unlock() }
        return body()
    }

    func resolveMx(domain: String) throws -> MxResolution {
        guard let answer = Self.query(domain, type: Int32(DnsMessage.typeMx)) else {
            throw DnsError.Lookup("mx lookup failed for \(domain)")
        }
        let parsed: DnsMessage.MxAnswer
        do {
            parsed = try DnsMessage.parseMxResponse(answer)
        } catch {
            throw DnsError.Lookup("malformed dns answer")
        }
        return MxResolution(
            records: parsed.records.map {
                MxRecord(preference: UInt16($0.preference), exchange: $0.exchange)
            },
            authenticData: parsed.authenticData
        )
    }

    func resolveSrv(name: String) throws -> SrvResolution {
        guard let answer = Self.query(name, type: Int32(DnsMessage.typeSrv)) else {
            throw DnsError.Lookup("srv lookup failed for \(name)")
        }
        let parsed: DnsMessage.SrvAnswer
        do {
            parsed = try DnsMessage.parseSrvResponse(answer)
        } catch {
            throw DnsError.Lookup("malformed dns answer")
        }
        return SrvResolution(
            records: parsed.records.map {
                SrvRecord(
                    priority: UInt16($0.priority),
                    weight: UInt16($0.weight),
                    port: UInt16($0.port),
                    target: $0.target
                )
            },
            authenticData: parsed.authenticData
        )
    }

    /// Sends one `res_9_query` for `name` (record `type`) under `queryLock`, returning the raw
    /// answer bytes, or `nil` when the lookup fails. The lock makes every libresolv query in the
    /// process mutually exclusive, the resolver crashes under concurrent use (see `queryLock`).
    private static func query(_ name: String, type: Int32) -> [UInt8]? {
        var answer = [UInt8](repeating: 0, count: 4096)
        let length = withQueryLock {
            name.withCString { cName -> Int32 in
                res_9_query(cName, Int32(DnsMessage.classIn), type, &answer, Int32(answer.count))
            }
        }
        guard length > 0 else { return nil }
        return Array(answer.prefix(Int(length)))
    }
}
