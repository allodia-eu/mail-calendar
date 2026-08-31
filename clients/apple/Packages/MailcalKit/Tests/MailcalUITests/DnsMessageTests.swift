// The DNS wire codec, golden query bytes, MX parsing with and without name compression, the
// AD bit both ways, and that malformed input throws rather than crashing. SystemMxResolver
// hands libresolv's raw answer to the same parser, but that needs a device; this is the bytes.

import Foundation
import Testing

@testable import MailcalUI

struct DnsMessageTests {
    @Test func buildsAnMxQueryWithTheAdBitSet() {
        let query = DnsMessage.buildMxQuery("example.com", id: 0x1234)
        #expect(query[0] == 0x12)
        #expect(query[1] == 0x34)
        #expect(query[2] == 0x01) // RD
        #expect(query[3] == 0x20) // AD
        #expect(Int(query[4]) << 8 | Int(query[5]) == 1) // QDCOUNT
        let name = Array(query[12...])
        let expected: [UInt8] = [
            7, 101, 120, 97, 109, 112, 108, 101, // "example"
            3, 99, 111, 109, // "com"
            0, 0, 15, 0, 1, // root, QTYPE MX, QCLASS IN
        ]
        #expect(name == expected)
    }

    @Test func parsesMxRecordsAndTheAdBit() throws {
        let msg = response(adBit: true, answers: [mxAnswer(10, name("mx1", "example", "com"))])
        let answer = try DnsMessage.parseMxResponse(msg)
        #expect(answer.authenticData)
        #expect(answer.records == [DnsMessage.MxEntry(preference: 10, exchange: "mx1.example.com")])
    }

    @Test func readsAnUnsetAdBit() throws {
        let msg = response(adBit: false, answers: [mxAnswer(5, name("mx", "example", "com"))])
        #expect(try DnsMessage.parseMxResponse(msg).authenticData == false)
    }

    @Test func followsACompressionPointer() throws {
        // The exchange ends in a pointer to the question name at offset 12.
        let exchange: [UInt8] = [2, 109, 120] + pointer(12) // "mx" + ->example.com
        let msg = response(adBit: true, answers: [mxAnswerRaw(20, exchange)])
        let answer = try DnsMessage.parseMxResponse(msg)
        #expect(answer.records.first?.exchange == "mx.example.com")
    }

    @Test func skipsNonMxAnswers() throws {
        let a = rawAnswer(type: 1, rdata: [93, 184, 216, 34]) // an A record
        let mx = mxAnswer(10, name("mx", "example", "com"))
        let msg = response(adBit: false, answers: [a, mx])
        let answer = try DnsMessage.parseMxResponse(msg)
        #expect(answer.records.count == 1)
        #expect(answer.records.first?.exchange == "mx.example.com")
    }

    @Test func truncatedMessageThrows() {
        let msg = response(adBit: true, answers: [mxAnswer(10, name("mx", "example", "com"))])
        #expect(throws: (any Error).self) {
            try DnsMessage.parseMxResponse(Array(msg[0 ..< (msg.count - 3)]))
        }
    }

    @Test func garbageDoesNotHang() {
        #expect(throws: (any Error).self) {
            try DnsMessage.parseMxResponse([UInt8](repeating: 0xFF, count: 12))
        }
    }

    @Test func buildsAnSrvQueryForAServiceName() {
        // Underscore service labels are ordinary DNS labels; QTYPE must be SRV (33).
        let query = DnsMessage.buildSrvQuery("_jmap._tcp.example.com", id: 0x0001)
        #expect(query[3] == 0x20) // AD requested
        let name = Array(query[12...])
        let expected: [UInt8] = [
            5, 95, 106, 109, 97, 112, // "_jmap"
            4, 95, 116, 99, 112, // "_tcp"
            7, 101, 120, 97, 109, 112, 108, 101, // "example"
            3, 99, 111, 109, // "com"
            0, 0, 33, 0, 1, // root, QTYPE SRV, QCLASS IN
        ]
        #expect(name == expected)
    }

    @Test func parsesSrvRecordsAndTheAdBit() throws {
        let msg = response(adBit: true, answers: [srvAnswer(0, 1, 993, name("imap", "example", "com"))])
        let answer = try DnsMessage.parseSrvResponse(msg)
        #expect(answer.authenticData)
        #expect(answer.records == [
            DnsMessage.SrvEntry(priority: 0, weight: 1, port: 993, target: "imap.example.com"),
        ])
    }

    @Test func readsAnSrvRootTargetAsEmpty() throws {
        // RFC 2782 ".": the service is explicitly not offered; target is just the root.
        let msg = response(adBit: false, answers: [srvAnswer(0, 0, 0, [0])])
        let answer = try DnsMessage.parseSrvResponse(msg)
        #expect(answer.records == [DnsMessage.SrvEntry(priority: 0, weight: 0, port: 0, target: "")])
    }

    // MARK: - wire builders

    private func name(_ labels: String...) -> [UInt8] {
        var out: [UInt8] = []
        for label in labels {
            let bytes = Array(label.utf8)
            out.append(UInt8(bytes.count))
            out.append(contentsOf: bytes)
        }
        out.append(0)
        return out
    }

    private func pointer(_ offset: Int) -> [UInt8] {
        [UInt8(0xC0 | (offset >> 8)), UInt8(offset & 0xFF)]
    }

    private func mxAnswer(_ preference: Int, _ exchange: [UInt8]) -> [UInt8] {
        mxAnswerRaw(preference, exchange)
    }

    private func mxAnswerRaw(_ preference: Int, _ exchangeBytes: [UInt8]) -> [UInt8] {
        let rdata = [UInt8(preference >> 8), UInt8(preference & 0xFF)] + exchangeBytes
        return rawAnswer(type: 15, rdata: rdata)
    }

    private func srvAnswer(_ priority: Int, _ weight: Int, _ port: Int, _ target: [UInt8]) -> [UInt8] {
        let rdata = [
            UInt8(priority >> 8), UInt8(priority & 0xFF),
            UInt8(weight >> 8), UInt8(weight & 0xFF),
            UInt8(port >> 8), UInt8(port & 0xFF),
        ] + target
        return rawAnswer(type: 33, rdata: rdata)
    }

    private func rawAnswer(type: Int, rdata: [UInt8]) -> [UInt8] {
        var out: [UInt8] = pointer(12) // NAME -> question
        out += [UInt8(type >> 8), UInt8(type & 0xFF)]
        out += [0, 1] // CLASS IN
        out += [0, 0, 0, 60] // TTL
        out += [UInt8(rdata.count >> 8), UInt8(rdata.count & 0xFF)]
        out += rdata
        return out
    }

    private func response(adBit: Bool, answers: [[UInt8]]) -> [UInt8] {
        var out: [UInt8] = [0, 0] // id
        out.append(0x81) // QR=1, RD=1
        out.append(adBit ? 0x20 : 0x00)
        out += [0, 1] // QDCOUNT
        out += [UInt8(answers.count >> 8), UInt8(answers.count & 0xFF)]
        out += [0, 0, 0, 0] // NS/AR counts
        out += name("example", "com")
        out += [0, 15, 0, 1] // QTYPE MX, QCLASS IN
        answers.forEach { out += $0 }
        return out
    }
}
