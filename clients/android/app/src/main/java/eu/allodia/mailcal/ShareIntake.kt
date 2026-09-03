// Turning a share Intent into a pre-filled composer: staging the shared bytes into app-private
// storage, then asking the shared core what they mean.
//
// Split from ShareLaunch.kt because this half needs a Context and the cdylib, and that one is a
// pure gate the JVM suite drives. See docs/os-integration.md.
package eu.allodia.mailcal

import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import java.io.File
import java.util.UUID
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.SharePrefill
import uniffi.mailcal_bindings.ShareRequest
import uniffi.mailcal_bindings.SharedFile
import uniffi.mailcal_bindings.prefillFromShare

// Reads a share and hands the result back on the main thread, or reports that it carried nothing
// to open a composer with.
//
// The copy runs off the main thread: a share of several photographs is megabytes of I/O, and a
// launch that froze the window while it copied would look like the app failing to start. That is
// also why this cannot simply return a value.
internal fun readShare(ctx: Context, intent: Intent, onReady: (SharePrefill) -> Unit) {
    val uris = ShareLaunch.sharedUris(intent)
    val text = ShareLaunch.sharedText(intent)
    val subject = ShareLaunch.sharedSubject(intent)
    thread(name = "mailcal-stage-share") {
        val files = uris.mapNotNull { uri -> stageSharedFile(ctx, uri) }
        // The core owns every decision from here: the names, the media types, the cap, and which
        // items it will not take. Nothing above this line inspected a file.
        val prefill = prefillFromShare(ShareRequest(files, text, subject))
        Handler(Looper.getMainLooper()).post { onReady(prefill) }
    }
}

// Copies one shared item into the app's own cache and describes it as the core expects.
//
// A `content://` URI is a grant to *this* launch, so the bytes are copied rather than referenced:
// the permission is gone by the time the user presses Send, and a path into another app's provider
// would not be readable by Rust in any case.
//
// The name and type are passed on **as the sending app gave them**, unsanitised, because
// sanitising is the core's job and doing it twice is how two answers appear for one file. The only
// cleaning here is of the *cache* filename, which is this app's own filesystem and not what the
// recipient sees.
private fun stageSharedFile(ctx: Context, uri: Uri): SharedFile? {
    val declaredName = displayName(ctx.contentResolver, uri)
    val dir = File(ctx.cacheDir, "shared-attachments").apply { mkdirs() }
    val staged = File(dir, "${UUID.randomUUID()}-${cacheFileName(declaredName)}")
    return try {
        ctx.contentResolver.openInputStream(uri)?.use { input ->
            staged.outputStream().use { output -> input.copyTo(output) }
        } ?: return null
        SharedFile(
            path = staged.absolutePath,
            suggestedName = declaredName,
            declaredMediaType = ctx.contentResolver.getType(uri).orEmpty(),
        )
    } catch (_: Exception) {
        // A provider that revoked the grant, a file that vanished, a sender that lied about what
        // it was offering. One unreadable item must not cost the user the rest of the share.
        staged.delete()
        null
    }
}

private fun displayName(resolver: ContentResolver, uri: Uri): String {
    resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
        val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
        if (index >= 0 && cursor.moveToFirst()) {
            cursor.getString(index)?.takeIf { it.isNotBlank() }?.let { return it }
        }
    }
    return uri.lastPathSegment.orEmpty()
}

// A name safe to create on this device's filesystem. Not the attachment's name: that one comes
// back from the core, already normalised, and is what the recipient reads.
private fun cacheFileName(value: String): String {
    val cleaned = value.map { ch ->
        if (ch.isISOControl() || ch in setOf('/', '\\', ':', '*', '?', '"', '<', '>', '|')) '_' else ch
    }.joinToString("").trim('.', ' ', '_').take(80)
    return cleaned.ifEmpty { "shared" }
}
