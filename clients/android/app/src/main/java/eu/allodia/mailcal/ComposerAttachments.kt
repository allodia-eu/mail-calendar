// Attachment staging for the rich composer: copying a user-picked `content://` document into the
// app cache so Rust can read its bytes off the main thread, plus the filename/media-type probing
// that feeds the outgoing MIME part. What is attached is the core's list, drawn by
// ComposerFileTiles.kt.
package eu.allodia.mailcal

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.activity.compose.ManagedActivityResultLauncher
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import java.io.File
import java.util.UUID
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.ComposerFileAttachment

// The Attach action's picker. Each chosen document is copied (content resolver → app cache) off
// the main thread, because a large selection would otherwise block the UI, and the result is
// applied back on it. `onFailed` covers a selection that staged nothing at all.
@Composable
internal fun rememberAttachmentPicker(
    onStaged: (List<ComposerFileAttachment>) -> Unit,
    onFailed: () -> Unit,
): ManagedActivityResultLauncher<Array<String>, List<Uri>> {
    val ctx = LocalContext.current
    return rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) {
            thread(name = "mailcal-stage-attachments") {
                val staged = uris.mapNotNull { uri -> stageComposerFile(ctx, uri) }
                Handler(Looper.getMainLooper()).post {
                    if (staged.isEmpty()) onFailed() else onStaged(staged)
                }
            }
        }
    }
}

// One picked or dropped document, copied where Rust can read it, as the core takes it.
internal fun stageComposerFile(ctx: Context, uri: Uri): ComposerFileAttachment? {
    val name = displayName(ctx, uri)
    val type = mediaType(ctx, uri, name)
    val dir = File(ctx.cacheDir, "composer-attachments").apply { mkdirs() }
    val file = File(dir, "${UUID.randomUUID()}-${safeFileName(name)}")
    return try {
        ctx.contentResolver.openInputStream(uri)?.use { input ->
            file.outputStream().use { output -> input.copyTo(output) }
        } ?: return null
        ComposerFileAttachment(path = file.absolutePath, fileName = name, mediaType = type)
    } catch (_: Exception) {
        file.delete()
        null
    }
}

private fun displayName(ctx: Context, uri: Uri): String {
    ctx.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor ->
            val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (index >= 0 && cursor.moveToFirst()) {
                cursor.getString(index)?.takeIf { it.isNotBlank() }?.let { return it }
            }
        }
    return uri.lastPathSegment?.substringAfterLast('/')?.takeIf { it.isNotBlank() } ?: "attachment"
}

private fun mediaType(ctx: Context, uri: Uri, fileName: String): String =
    ctx.contentResolver.getType(uri)
        ?: MimeTypeMap.getSingleton()
            .getMimeTypeFromExtension(fileName.substringAfterLast('.', "").lowercase())
        ?: "application/octet-stream"

private fun safeFileName(value: String): String {
    val cleaned = value.map { ch ->
        if (ch.isISOControl() || ch in setOf('/', '\\', ':', '*', '?', '"', '<', '>', '|')) {
            '_'
        } else {
            ch
        }
    }.joinToString("").trim('.', ' ', '_')
    return cleaned.ifEmpty { "attachment" }
}
