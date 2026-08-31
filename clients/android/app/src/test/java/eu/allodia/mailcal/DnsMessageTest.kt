// JVM tests for the DNS wire codec, golden query bytes, MX parsing with and without
// name compression, the AD bit both ways, and that malformed input throws rather than
// crashing or looping. No device or network needed.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DnsMessageTest {
    @Test
    fun buildsAnMxQueryWithTheAdBitSet() {
        val query = DnsMessage.buildMxQuery("example.com", id = 0x1234)
        // Header: id, flags RD|AD, QDCOUNT=1.
        assertEquals(0x12, query[0].toInt() and 0xFF)
        assertEquals(0x34, query[1].toInt() and 0xFF)
        assertEquals(0x01, query[2].toInt() and 0xFF) // RD high byte
        assertEquals(0x20, query[3].toInt() and 0xFF) // AD low byte
        assertEquals(0x0001, ((query[4].toInt() and 0xFF) shl 8) or (query[5].toInt() and 0xFF))
        // Question name: 7"example" 3"com" 0, then QTYPE=15, QCLASS=1.
        val name = query.copyOfRange(12, query.size)
        val expected = byteArrayOf(
            7, 'e'.code.toByte(), 'x'.code.toByte(), 'a'.code.toByte(), 'm'.code.toByte(),
            'p'.code.toByte(), 'l'.code.toByte(), 'e'.code.toByte(),
            3, 'c'.code.toByte(), 'o'.code.toByte(), 'm'.code.toByte(),
            0, 0, 15, 0, 1,
        )
        assertEquals(expected.toList(), name.toList())
    }

    @Test(expected = IllegalArgumentException::class)
    fun rejectsAnEmptyDomain() {
        DnsMessage.buildMxQuery("")
    }

    @Test
    fun parsesMxRecordsAndTheAdBit() {
        val msg = response(
            adBit = true,
            question = name("example", "com"),
            answers = listOf(mxAnswer(10, name("mx1", "example", "com"))),
        )
        val answer = DnsMessage.parseMxResponse(msg)
        assertTrue(answer.authenticData)
        assertEquals(1, answer.records.size)
        assertEquals(10, answer.records[0].preference)
        assertEquals("mx1.example.com", answer.records[0].exchange)
    }

    @Test
    fun readsAnUnsetAdBit() {
        val msg = response(
            adBit = false,
            question = name("example", "com"),
            answers = listOf(mxAnswer(5, name("mx", "example", "com"))),
        )
        assertFalse(DnsMessage.parseMxResponse(msg).authenticData)
    }

    @Test
    fun followsACompressionPointer() {
        // The exchange name ends in a pointer back to "example.com" in the question.
        // The question "example.com" starts at offset 12.
        val exchangePrefix = byteArrayOf(2, 'm'.code.toByte(), 'x'.code.toByte()) + pointer(12)
        val msg = response(
            adBit = true,
            question = name("example", "com"),
            answers = listOf(mxAnswerRaw(20, exchangePrefix)),
        )
        val answer = DnsMessage.parseMxResponse(msg)
        assertEquals("mx.example.com", answer.records[0].exchange)
    }

    @Test
    fun skipsNonMxAnswers() {
        val a = byteArrayOf(93, 184.toByte(), 216.toByte(), 34) // an A record's 4-byte rdata
        val msg = response(
            adBit = false,
            question = name("example", "com"),
            answers = listOf(rawAnswer(type = 1, rdata = a), mxAnswer(10, name("mx", "example", "com"))),
        )
        val answer = DnsMessage.parseMxResponse(msg)
        assertEquals(1, answer.records.size)
        assertEquals("mx.example.com", answer.records[0].exchange)
    }

    @Test(expected = IllegalArgumentException::class)
    fun truncatedMessageThrows() {
        val msg = response(
            adBit = true,
            question = name("example", "com"),
            answers = listOf(mxAnswer(10, name("mx", "example", "com"))),
        )
        DnsMessage.parseMxResponse(msg.copyOfRange(0, msg.size - 3))
    }

    @Test(expected = IllegalArgumentException::class)
    fun garbageDoesNotHang() {
        DnsMessage.parseMxResponse(ByteArray(12) { 0xFF.toByte() })
    }

    @Test
    fun buildsAnSrvQueryForAServiceName() {
        // Underscore service labels are ordinary DNS labels; QTYPE must be SRV (33).
        val query = DnsMessage.buildSrvQuery("_jmap._tcp.example.com", id = 0x0001)
        assertEquals(0x20, query[3].toInt() and 0xFF) // AD bit requested
        val name = query.copyOfRange(12, query.size)
        val expected = byteArrayOf(
            5, '_'.code.toByte(), 'j'.code.toByte(), 'm'.code.toByte(), 'a'.code.toByte(), 'p'.code.toByte(),
            4, '_'.code.toByte(), 't'.code.toByte(), 'c'.code.toByte(), 'p'.code.toByte(),
            7, 'e'.code.toByte(), 'x'.code.toByte(), 'a'.code.toByte(), 'm'.code.toByte(),
            'p'.code.toByte(), 'l'.code.toByte(), 'e'.code.toByte(),
            3, 'c'.code.toByte(), 'o'.code.toByte(), 'm'.code.toByte(),
            0, 0, 33, 0, 1, // QTYPE=SRV, QCLASS=IN
        )
        assertEquals(expected.toList(), name.toList())
    }

    @Test
    fun parsesSrvRecordsAndTheAdBit() {
        val msg = response(
            adBit = true,
            question = name("_imaps", "_tcp", "example", "com"),
            answers = listOf(srvAnswer(0, 1, 993, name("imap", "example", "com"))),
            qtype = 33,
        )
        val answer = DnsMessage.parseSrvResponse(msg)
        assertTrue(answer.authenticData)
        assertEquals(1, answer.records.size)
        val record = answer.records[0]
        assertEquals(0, record.priority)
        assertEquals(1, record.weight)
        assertEquals(993, record.port)
        assertEquals("imap.example.com", record.target)
    }

    @Test
    fun readsAnSrvRootTargetAsEmpty() {
        // RFC 2782 ".": the service is explicitly not offered; the target is just the root.
        val msg = response(
            adBit = false,
            question = name("_submissions", "_tcp", "example", "com"),
            answers = listOf(srvAnswer(0, 0, 0, byteArrayOf(0))),
            qtype = 33,
        )
        val answer = DnsMessage.parseSrvResponse(msg)
        assertEquals(1, answer.records.size)
        assertEquals("", answer.records[0].target)
    }

    // --- wire builders for the fixtures ---

    private fun name(vararg labels: String): ByteArray {
        val out = ArrayList<Byte>()
        for (label in labels) {
            out.add(label.length.toByte())
            out.addAll(label.toByteArray(Charsets.US_ASCII).toList())
        }
        out.add(0)
        return out.toByteArray()
    }

    private fun pointer(offset: Int): ByteArray =
        byteArrayOf((0xC0 or (offset ushr 8)).toByte(), (offset and 0xFF).toByte())

    private fun mxAnswer(preference: Int, exchange: ByteArray): ByteArray =
        mxAnswerRaw(preference, exchange)

    private fun mxAnswerRaw(preference: Int, exchangeBytes: ByteArray): ByteArray {
        val rdata = byteArrayOf((preference ushr 8).toByte(), (preference and 0xFF).toByte()) + exchangeBytes
        return rawAnswer(type = 15, rdata = rdata)
    }

    private fun srvAnswer(priority: Int, weight: Int, port: Int, target: ByteArray): ByteArray {
        val rdata = byteArrayOf(
            (priority ushr 8).toByte(), (priority and 0xFF).toByte(),
            (weight ushr 8).toByte(), (weight and 0xFF).toByte(),
            (port ushr 8).toByte(), (port and 0xFF).toByte(),
        ) + target
        return rawAnswer(type = 33, rdata = rdata)
    }

    // An answer RR with a root NAME pointer-free (just a compression pointer to the question)
    // to keep fixtures small; TYPE/CLASS/TTL/RDLENGTH + rdata.
    private fun rawAnswer(type: Int, rdata: ByteArray): ByteArray {
        val out = ArrayList<Byte>()
        out.addAll(pointer(12).toList()) // NAME → the question name
        out.add((type ushr 8).toByte()); out.add((type and 0xFF).toByte())
        out.add(0); out.add(1) // CLASS IN
        out.addAll(listOf(0, 0, 0, 60).map { it.toByte() }) // TTL
        out.add((rdata.size ushr 8).toByte()); out.add((rdata.size and 0xFF).toByte())
        out.addAll(rdata.toList())
        return out.toByteArray()
    }

    private fun response(
        adBit: Boolean,
        question: ByteArray,
        answers: List<ByteArray>,
        qtype: Int = 15,
    ): ByteArray {
        val out = ArrayList<Byte>()
        out.add(0); out.add(0) // id
        out.add(0x81.toByte()) // QR=1, RD=1
        out.add((if (adBit) 0x20 else 0x00).toByte()) // AD bit
        out.add(0); out.add(1) // QDCOUNT
        out.add((answers.size ushr 8).toByte()); out.add((answers.size and 0xFF).toByte())
        out.add(0); out.add(0) // NSCOUNT
        out.add(0); out.add(0) // ARCOUNT
        out.addAll(question.toList())
        out.add((qtype ushr 8).toByte()); out.add((qtype and 0xFF).toByte()); out.add(0); out.add(1) // QTYPE, QCLASS IN
        answers.forEach { out.addAll(it.toList()) }
        return out.toByteArray()
    }
}
