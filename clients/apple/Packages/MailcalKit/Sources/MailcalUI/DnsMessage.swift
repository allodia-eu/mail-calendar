// A tiny DNS wire-format codec for MX lookups, the same shape as the Android DnsMessage,
// kept free of any system-resolver dependency so the package test suite covers it. The
// SystemMxResolver hands libresolv's raw answer bytes to `parseMxResponse`; this file is
// only the bytes: build an MX query (AD bit set) and read MX records + the AD bit back out,
// following name-compression pointers (RFC 1035 §4.1.4). Malformed input throws.
import Foundation

enum DnsMessageError: Error {
    case truncated
    case compressionLoop
}

enum DnsMessage {
    static let typeMx = 15
    static let typeSrv = 33
    static let classIn = 1

    // One MX record: a preference (lower is preferred) and the mail-exchange hostname.
    struct MxEntry: Equatable {
        let preference: Int
        let exchange: String
    }

    // A parsed MX response: the records, and whether the resolver set the AD (authentic
    // data) header bit, i.e. it DNSSEC-validated the answer.
    struct MxAnswer: Equatable {
        let records: [MxEntry]
        let authenticData: Bool
    }

    // One SRV record (RFC 2782): priority (lower preferred), weight, port, target host.
    struct SrvEntry: Equatable {
        let priority: Int
        let weight: Int
        let port: Int
        let target: String
    }

    // A parsed SRV response: the records and the AD header bit.
    struct SrvAnswer: Equatable {
        let records: [SrvEntry]
        let authenticData: Bool
    }

    // Builds a DNS query for the MX records of `domain`, with RD and AD flags set.
    static func buildMxQuery(_ domain: String, id: UInt16 = 0) -> [UInt8] {
        buildQuery(domain, type: typeMx, id: id)
    }

    // Builds a DNS query for the SRV records of `name` (a service owner name like
    // "_jmap._tcp.example.com", underscore labels are ordinary DNS labels here).
    static func buildSrvQuery(_ name: String, id: UInt16 = 0) -> [UInt8] {
        buildQuery(name, type: typeSrv, id: id)
    }

    // Builds a DNS query for `name` of record `type`, with RD and AD flags set.
    private static func buildQuery(_ name: String, type: Int, id: UInt16) -> [UInt8] {
        var out: [UInt8] = []
        out.append(UInt8(id >> 8))
        out.append(UInt8(id & 0xFF))
        out.append(0x01) // RD (high flags byte)
        out.append(0x20) // AD (low flags byte)
        out.append(0x00)
        out.append(0x01) // QDCOUNT = 1
        out.append(contentsOf: [0, 0, 0, 0, 0, 0]) // AN/NS/AR counts
        for label in name.split(separator: ".", omittingEmptySubsequences: true) {
            let bytes = Array(label.utf8)
            out.append(UInt8(bytes.count))
            out.append(contentsOf: bytes)
        }
        out.append(0) // root label
        out.append(0x00)
        out.append(UInt8(type))
        out.append(0x00)
        out.append(UInt8(classIn))
        return out
    }

    // Parses an MX response: reads the AD bit, skips the question section, and collects
    // every MX answer record (decompressing exchange names). Non-MX answers are skipped.
    static func parseMxResponse(_ msg: [UInt8]) throws -> MxAnswer {
        guard msg.count >= 12 else { throw DnsMessageError.truncated }
        let authenticData = (try u8(msg, 3) & 0x20) != 0
        let questionCount = try u16(msg, 4)
        let answerCount = try u16(msg, 6)

        var pos = 12
        for _ in 0 ..< questionCount {
            pos = try skipName(msg, pos) + 4 // QTYPE + QCLASS
        }

        var records: [MxEntry] = []
        for _ in 0 ..< answerCount {
            pos = try skipName(msg, pos)
            let type = try u16(msg, pos)
            let rdLength = try u16(msg, pos + 8)
            pos += 10 // TYPE(2) + CLASS(2) + TTL(4) + RDLENGTH(2)
            if type == typeMx {
                let preference = try u16(msg, pos)
                let (exchange, _) = try readName(msg, pos + 2)
                records.append(MxEntry(preference: preference, exchange: exchange))
            }
            pos += rdLength
        }
        return MxAnswer(records: records, authenticData: authenticData)
    }

    // Parses an SRV response: like `parseMxResponse`, but each SRV rdata is priority(2) +
    // weight(2) + port(2) + a (possibly compressed) target name. Non-SRV answers are
    // skipped; the RFC 2782 root target "." reads back as an empty string.
    static func parseSrvResponse(_ msg: [UInt8]) throws -> SrvAnswer {
        guard msg.count >= 12 else { throw DnsMessageError.truncated }
        let authenticData = (try u8(msg, 3) & 0x20) != 0
        let questionCount = try u16(msg, 4)
        let answerCount = try u16(msg, 6)

        var pos = 12
        for _ in 0 ..< questionCount {
            pos = try skipName(msg, pos) + 4 // QTYPE + QCLASS
        }

        var records: [SrvEntry] = []
        for _ in 0 ..< answerCount {
            pos = try skipName(msg, pos)
            let type = try u16(msg, pos)
            let rdLength = try u16(msg, pos + 8)
            pos += 10 // TYPE(2) + CLASS(2) + TTL(4) + RDLENGTH(2)
            if type == typeSrv {
                let priority = try u16(msg, pos)
                let weight = try u16(msg, pos + 2)
                let port = try u16(msg, pos + 4)
                let (target, _) = try readName(msg, pos + 6)
                records.append(SrvEntry(priority: priority, weight: weight, port: port, target: target))
            }
            pos += rdLength
        }
        return SrvAnswer(records: records, authenticData: authenticData)
    }

    private static func u8(_ msg: [UInt8], _ index: Int) throws -> Int {
        guard index >= 0, index < msg.count else { throw DnsMessageError.truncated }
        return Int(msg[index])
    }

    private static func u16(_ msg: [UInt8], _ index: Int) throws -> Int {
        try (u8(msg, index) << 8) | u8(msg, index + 1)
    }

    // Advances past a name without expanding it (a pointer ends it in two bytes).
    private static func skipName(_ msg: [UInt8], _ start: Int) throws -> Int {
        var pos = start
        while true {
            let length = try u8(msg, pos)
            if length == 0 { return pos + 1 }
            if (length & 0xC0) == 0xC0 { return pos + 2 }
            pos += 1 + length
        }
    }

    // Reads a (possibly compressed) name, returning the dotted string and the offset just
    // past the name in its original position.
    private static func readName(_ msg: [UInt8], _ start: Int) throws -> (String, Int) {
        var labels: [String] = []
        var pos = start
        var next = start
        var jumped = false
        var guardCount = 0
        while true {
            guardCount += 1
            if guardCount >= 128 { throw DnsMessageError.compressionLoop }
            let length = try u8(msg, pos)
            if length == 0 {
                if !jumped { next = pos + 1 }
                return (labels.joined(separator: "."), next)
            }
            if (length & 0xC0) == 0xC0 {
                let pointer = ((length & 0x3F) << 8) | (try u8(msg, pos + 1))
                if !jumped { next = pos + 2 }
                jumped = true
                pos = pointer
            } else {
                guard pos + 1 + length <= msg.count else { throw DnsMessageError.truncated }
                labels.append(String(decoding: msg[(pos + 1) ..< (pos + 1 + length)], as: UTF8.self))
                pos += 1 + length
            }
        }
    }
}
