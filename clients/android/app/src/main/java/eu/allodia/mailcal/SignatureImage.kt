// A picture picked for a signature (docs/signatures.md, "What a signature may hold"), read into a
// self-contained `data:` URI. The core's rule decides what may go in: it is asked with the media
// type and size the provider reports before a byte is read, and again as the bytes arrive, so a
// file that reports no size, or the wrong one, is never read far past what the rule allows.
package eu.allodia.mailcal

import android.content.ContentResolver
import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.util.Base64
import uniffi.mailcal_bindings.SignatureImageRefusal

/** What the signature image picker offers: the three types every recipient's mail app shows. */
internal val SIGNATURE_IMAGE_TYPES = arrayOf("image/png", "image/jpeg", "image/gif")

/** How much of a picture is read between two askings of the rule. */
internal const val SIGNATURE_IMAGE_CHUNK = 64 * 1024

internal sealed interface SignatureImage {
    data class DataUrl(val value: String, val altText: String) : SignatureImage

    data class Refused(val refusal: SignatureImageRefusal) : SignatureImage

    data object Failed : SignatureImage
}

/**
 * Reads the picture [open] yields, of [mediaType] and the [size] its provider reports (null when it
 * reports none), asking [refusal] before the first byte and after each chunk.
 */
internal fun signatureImage(
    mediaType: String?,
    size: Long?,
    altText: String,
    refusal: (mediaType: String, size: Long) -> SignatureImageRefusal?,
    open: () -> InputStream?,
): SignatureImage {
    val type = mediaType.orEmpty()
    refusal(type, size ?: 0L)?.let { return SignatureImage.Refused(it) }
    val bytes = ByteArrayOutputStream()
    val buffer = ByteArray(SIGNATURE_IMAGE_CHUNK)
    (open() ?: return SignatureImage.Failed).use { input ->
        while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            bytes.write(buffer, 0, read)
            refusal(type, bytes.size().toLong())?.let { return SignatureImage.Refused(it) }
        }
    }
    if (bytes.size() == 0) return SignatureImage.Failed
    val encoded = Base64.getEncoder().encodeToString(bytes.toByteArray())
    return SignatureImage.DataUrl("data:$type;base64,$encoded", altText)
}

/** Reads the picture at [uri] for a signature, by the core's [refusal]. */
internal fun readSignatureImage(
    ctx: Context,
    uri: Uri,
    refusal: (mediaType: String, size: Long) -> SignatureImageRefusal?,
): SignatureImage = try {
    val resolver = ctx.contentResolver
    signatureImage(
        mediaType = resolver.getType(uri),
        size = reportedSize(resolver, uri),
        altText = uri.lastPathSegment?.substringAfterLast('/').orEmpty(),
        refusal = refusal,
        open = { resolver.openInputStream(uri) },
    )
} catch (_: Exception) {
    SignatureImage.Failed
}

// The size the provider reports, from its OpenableColumns.SIZE or else its file descriptor, or null
// when neither says.
private fun reportedSize(resolver: ContentResolver, uri: Uri): Long? {
    resolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { cursor ->
        val column = cursor.getColumnIndex(OpenableColumns.SIZE)
        if (column >= 0 && cursor.moveToFirst() && !cursor.isNull(column)) return cursor.getLong(column)
    }
    return resolver.openAssetFileDescriptor(uri, "r")?.use { it.length }?.takeIf { it >= 0 }
}
