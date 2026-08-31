// The per-image cap and the media-type refusal on a signature's embedded logo (docs/signatures.md).
// Both are enforced where the file is picked, the core does not police them, so this is the only
// place either can be checked, and neither is visible until it is wrong: an uncapped logo rides in
// EVERY message the account sends, and a non-image `data:` URI is silently dropped by the editor
// with no message to the user.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SignatureImageTest {

    @Test
    fun `an image under the cap becomes a self-contained data URI`() {
        val outcome = signatureImageDataUrl(byteArrayOf(1, 2, 3), "image/png", altText = "logo.png")

        val url = (outcome as SignatureImage.DataUrl).value
        assertTrue("the editor only accepts data:image/", url.startsWith("data:image/png;base64,"))
        assertEquals("AQID", url.substringAfter("base64,"))
        assertEquals("logo.png", outcome.altText)
    }

    /**
     * A signature rides in every message the account sends, and base64 adds a third on top, so the
     * cap is about what leaves the device, not about what the editor can hold.
     */
    @Test
    fun `an image over the cap is refused, and says what the cap is`() {
        val outcome = signatureImageDataUrl(
            ByteArray(SIGNATURE_IMAGE_LIMIT_BYTES + 1),
            "image/png",
            altText = "huge.png",
        )

        assertEquals(SignatureImage.TooLarge(SIGNATURE_IMAGE_LIMIT_BYTES), outcome)
    }

    @Test
    fun `exactly the cap is still accepted`() {
        val outcome = signatureImageDataUrl(ByteArray(SIGNATURE_IMAGE_LIMIT_BYTES), "image/png", "logo.png")

        assertTrue(outcome is SignatureImage.DataUrl)
    }

    /**
     * Refused at the picker rather than embedded: the editor's own `data:image/` check would drop it
     * silently, and the picker is where the user can still be told why nothing appeared. A
     * `data:text/html` in particular would be an executable document.
     */
    @Test
    fun `anything that is not an image is refused`() {
        assertEquals(
            SignatureImage.Failed,
            signatureImageDataUrl(byteArrayOf(1), "text/html", altText = "page"),
        )
        assertEquals(
            SignatureImage.Failed,
            signatureImageDataUrl(byteArrayOf(1), mediaType = null, altText = "unknown"),
        )
        assertEquals(
            "an empty read is a failure, not an empty image",
            SignatureImage.Failed,
            signatureImageDataUrl(ByteArray(0), "image/png", altText = "empty.png"),
        )
    }
}
