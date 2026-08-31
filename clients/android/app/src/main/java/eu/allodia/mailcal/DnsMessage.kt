// A tiny DNS wire-format codec for MX lookups, a plain query builder and response parser
// over ByteArray, with zero Android or FFI imports so it runs under the JVM test suite
// (Robolectric has no network and android.net.DnsResolver needs a device). DnsMx wraps the
// platform resolver around it; this file is only the bytes.
//
// Only what the MX fallback needs: build an MX query with the AD bit set (asking a
// validating resolver to report DNSSEC authentication), and read MX records + the AD bit
// back out, following name-compression pointers (RFC 1035 §4.1.4). Malformed or truncated
// input throws rather than looping or reading out of bounds.
package eu.allodia.mailcal

import java.io.ByteArrayOutputStream

internal object DnsMessage {
    private const val TYPE_MX = 15
    private const val TYPE_SRV = 33
    private const val CLASS_IN = 1

    // One MX record: a preference (lower is preferred) and the mail-exchange hostname.
    internal data class MxEntry(val preference: Int, val exchange: String)

    // A parsed MX response: the records, and whether the resolver set the AD (authentic
    // data) header bit, i.e. it DNSSEC-validated the answer.
    internal data class MxAnswer(val records: List<MxEntry>, val authenticData: Boolean)

    // One SRV record (RFC 2782): priority (lower preferred), weight, port, and target host.
    internal data class SrvEntry(val priority: Int, val weight: Int, val port: Int, val target: String)

    // A parsed SRV response: the records and the AD header bit.
    internal data class SrvAnswer(val records: List<SrvEntry>, val authenticData: Boolean)

    // Builds a DNS query for the MX records of [domain]. See [buildQuery].
    fun buildMxQuery(domain: String, id: Int = 0): ByteArray = buildQuery(domain, TYPE_MX, id)

    // Builds a DNS query for the SRV records of [name] (a service owner name like
    // "_jmap._tcp.example.com", underscore labels are ordinary DNS labels here).
    fun buildSrvQuery(name: String, id: Int = 0): ByteArray = buildQuery(name, TYPE_SRV, id)

    // Builds a DNS query for [name] of record [type]. Flags set RD (recursion desired) and
    // AD (request DNSSEC-authenticated data); [id] is fixed by default so the bytes are
    // deterministic for tests (the platform resolver owns transport-level anti-spoofing).
    private fun buildQuery(name: String, type: Int, id: Int): ByteArray {
        val labels = name.trimEnd('.').split('.')
        require(labels.isNotEmpty() && labels.all { it.isNotEmpty() }) { "invalid name: $name" }

        val out = ByteArrayOutputStream()
        out.write((id ushr 8) and 0xFF)
        out.write(id and 0xFF)
        // Flags: 0x0120 = RD (0x0100) | AD (0x0020).
        out.write(0x01)
        out.write(0x20)
        // QDCOUNT = 1; ANCOUNT/NSCOUNT/ARCOUNT = 0.
        out.write(0x00)
        out.write(0x01)
        repeat(6) { out.write(0x00) }
        for (label in labels) {
            val bytes = label.toByteArray(Charsets.US_ASCII)
            require(bytes.size in 1..63) { "invalid label: $label" }
            out.write(bytes.size)
            out.write(bytes)
        }
        out.write(0x00) // root label terminates the name
        out.write((type ushr 8) and 0xFF)
        out.write(type and 0xFF)
        out.write((CLASS_IN ushr 8) and 0xFF)
        out.write(CLASS_IN and 0xFF)
        return out.toByteArray()
    }

    // Parses an MX response. Reads the AD bit from the header, skips the question section,
    // and collects every MX answer record (decompressing exchange names). Non-MX answers
    // are skipped. Throws on truncation or a compression loop.
    fun parseMxResponse(msg: ByteArray): MxAnswer {
        require(msg.size >= 12) { "short header" }
        val authenticData = (u8(msg, 3) and 0x20) != 0
        val questionCount = u16(msg, 4)
        val answerCount = u16(msg, 6)

        var pos = 12
        repeat(questionCount) {
            pos = skipName(msg, pos)
            pos += 4 // QTYPE + QCLASS
        }

        val records = ArrayList<MxEntry>()
        repeat(answerCount) {
            pos = skipName(msg, pos)
            val type = u16(msg, pos)
            val rdLength = u16(msg, pos + 8)
            pos += 10 // TYPE(2) + CLASS(2) + TTL(4) + RDLENGTH(2)
            if (type == TYPE_MX) {
                val preference = u16(msg, pos)
                val (exchange, _) = readName(msg, pos + 2)
                records.add(MxEntry(preference, exchange))
            }
            pos += rdLength
        }
        return MxAnswer(records, authenticData)
    }

    // Parses an SRV response. Like [parseMxResponse], but each SRV rdata is priority(2) +
    // weight(2) + port(2) + a (possibly compressed) target name. Non-SRV answers are
    // skipped; the RFC 2782 root target "." reads back as an empty string.
    fun parseSrvResponse(msg: ByteArray): SrvAnswer {
        require(msg.size >= 12) { "short header" }
        val authenticData = (u8(msg, 3) and 0x20) != 0
        val questionCount = u16(msg, 4)
        val answerCount = u16(msg, 6)

        var pos = 12
        repeat(questionCount) {
            pos = skipName(msg, pos)
            pos += 4 // QTYPE + QCLASS
        }

        val records = ArrayList<SrvEntry>()
        repeat(answerCount) {
            pos = skipName(msg, pos)
            val type = u16(msg, pos)
            val rdLength = u16(msg, pos + 8)
            pos += 10 // TYPE(2) + CLASS(2) + TTL(4) + RDLENGTH(2)
            if (type == TYPE_SRV) {
                val priority = u16(msg, pos)
                val weight = u16(msg, pos + 2)
                val port = u16(msg, pos + 4)
                val (target, _) = readName(msg, pos + 6)
                records.add(SrvEntry(priority, weight, port, target))
            }
            pos += rdLength
        }
        return SrvAnswer(records, authenticData)
    }

    // A byte as an unsigned int, bounds-checked.
    private fun u8(msg: ByteArray, index: Int): Int {
        require(index in msg.indices) { "read past end of message" }
        return msg[index].toInt() and 0xFF
    }

    // A big-endian 16-bit value, bounds-checked.
    private fun u16(msg: ByteArray, index: Int): Int = (u8(msg, index) shl 8) or u8(msg, index + 1)

    // Advances past a name without expanding it: a pointer (top two bits set) ends the name
    // in two bytes; a zero-length label terminates; otherwise skip the label.
    private fun skipName(msg: ByteArray, start: Int): Int {
        var pos = start
        while (true) {
            val length = u8(msg, pos)
            when {
                length == 0 -> return pos + 1
                (length and 0xC0) == 0xC0 -> return pos + 2
                else -> pos += 1 + length
            }
        }
    }

    // Reads a (possibly compressed) name, returning the dotted string and the offset just
    // past the name in its original position. A guard bounds pointer-following.
    private fun readName(msg: ByteArray, start: Int): Pair<String, Int> {
        val labels = ArrayList<String>()
        var pos = start
        var next = start
        var jumped = false
        var guard = 0
        while (true) {
            require(guard++ < 128) { "name compression loop" }
            val length = u8(msg, pos)
            when {
                length == 0 -> {
                    if (!jumped) next = pos + 1
                    return Pair(labels.joinToString("."), next)
                }
                (length and 0xC0) == 0xC0 -> {
                    val pointer = ((length and 0x3F) shl 8) or u8(msg, pos + 1)
                    if (!jumped) next = pos + 2
                    jumped = true
                    pos = pointer
                }
                else -> {
                    require(pos + 1 + length <= msg.size) { "label past end" }
                    labels.add(String(msg, pos + 1, length, Charsets.US_ASCII))
                    pos += 1 + length
                }
            }
        }
    }
}
