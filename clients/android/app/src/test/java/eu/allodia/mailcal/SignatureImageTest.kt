// Reading a picture picked for a signature (docs/signatures.md, "What a signature may hold"). What
// may go in is the core's rule, stood in for here by `fakeImageRefusal`; what is Android's is when
// it is asked: with the type and size the provider reports before a byte is read, and again as the
// bytes arrive, so a file that reports no size is never read far past the limit.
package eu.allodia.mailcal

import java.io.ByteArrayInputStream
import java.io.InputStream
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.SignatureImageRefusal

class SignatureImageTest {

    /** A stream of zeros that never ends, counting what was read from it. */
    private class Endless : InputStream() {
        var served = 0L

        override fun read(): Int {
            served++
            return 0
        }

        override fun read(b: ByteArray, off: Int, len: Int): Int {
            b.fill(0, off, off + len)
            served += len
            return len
        }
    }

    private var opened = 0

    private fun read(
        mediaType: String?,
        size: Long?,
        asked: MutableList<Pair<String, Long>> = mutableListOf(),
        stream: () -> InputStream? = { ByteArrayInputStream(byteArrayOf(1, 2, 3)) },
    ) = signatureImage(
        mediaType = mediaType,
        size = size,
        altText = "logo.png",
        refusal = { type, bytes ->
            asked += type to bytes
            fakeImageRefusal(type, bytes)
        },
        open = {
            opened++
            stream()
        },
    )

    @Test
    fun `an accepted picture becomes a self-contained data URI of the type reported`() {
        val outcome = read("image/png", size = 3)

        assertEquals(SignatureImage.DataUrl("data:image/png;base64,AQID", "logo.png"), outcome)
    }

    @Test
    fun `a picture reported too large is refused before it is opened`() {
        val outcome = read("image/png", size = FAKE_SIGNATURE_IMAGE_LIMIT + 1)

        assertEquals(SignatureImage.Refused(SignatureImageRefusal.TooLarge(FAKE_SIGNATURE_IMAGE_LIMIT.toULong())), outcome)
        assertEquals(0, opened)
    }

    @Test
    fun `a type the rule refuses is refused before it is opened`() {
        assertEquals(SignatureImage.Refused(SignatureImageRefusal.Format), read("image/svg+xml", size = 3))
        assertEquals(SignatureImage.Refused(SignatureImageRefusal.Format), read(mediaType = null, size = 3))
        assertEquals(0, opened)
    }

    /** The rule decides about case, so the type goes to it as the provider reported it. */
    @Test
    fun `the type reaches the rule as the provider reported it`() {
        val asked = mutableListOf<Pair<String, Long>>()
        read("IMAGE/PNG", size = 3, asked = asked)

        assertEquals("IMAGE/PNG" to 3L, asked.first())
    }

    @Test
    fun `a picture that reports no size stops being read once the rule refuses it`() {
        val stream = Endless()
        val outcome = read("image/png", size = null, stream = { stream })

        assertEquals(SignatureImage.Refused(SignatureImageRefusal.TooLarge(FAKE_SIGNATURE_IMAGE_LIMIT.toULong())), outcome)
        assertTrue(stream.served <= FAKE_SIGNATURE_IMAGE_LIMIT + SIGNATURE_IMAGE_CHUNK)
    }

    @Test
    fun `a picture larger than it reported is refused as the bytes arrive`() {
        val outcome = read("image/png", size = 10, stream = { Endless() })

        assertTrue(outcome is SignatureImage.Refused)
    }

    @Test
    fun `nothing to read is a failure, not an empty picture`() {
        assertEquals(SignatureImage.Failed, read("image/png", size = 0, stream = { ByteArrayInputStream(ByteArray(0)) }))
        assertEquals(SignatureImage.Failed, read("image/png", size = 3, stream = { null }))
    }
}
