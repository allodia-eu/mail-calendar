// Attachment staging for the rich composer: copying a user-picked `content://` document into the
// app cache so Rust can read its bytes off the main thread, plus the filename/media-type probing
// that feeds the outgoing MIME part. Split out of RichComposeScreen.kt to keep each file small;
// behaviour is unchanged, the code is only relocated.
package eu.allodia.mailcal

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.activity.compose.ManagedActivityResultLauncher
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import java.io.File
import java.util.UUID
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.ComposerFileAttachment

internal data class PickedComposerFile(
    val id: String,
    val path: String,
    val fileName: String,
    val mediaType: String,
) {
    val composerFile: ComposerFileAttachment
        get() = ComposerFileAttachment(path, fileName, mediaType)
}

// The Attach action's picker. Each chosen document is copied (content resolver → app cache) off
// the main thread, because a large selection would otherwise block the UI, and the result is
// applied back on it. `onFailed` covers a selection that staged nothing at all.
@Composable
internal fun rememberAttachmentPicker(
    onStaged: (List<PickedComposerFile>) -> Unit,
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

internal fun stageComposerFile(ctx: Context, uri: Uri): PickedComposerFile? {
    val name = displayName(ctx, uri)
    val type = mediaType(ctx, uri, name)
    val dir = File(ctx.cacheDir, "composer-attachments").apply { mkdirs() }
    val file = File(dir, "${UUID.randomUUID()}-${safeFileName(name)}")
    return try {
        ctx.contentResolver.openInputStream(uri)?.use { input ->
            file.outputStream().use { output -> input.copyTo(output) }
        } ?: return null
        PickedComposerFile(
            id = UUID.randomUUID().toString(),
            path = file.absolutePath,
            fileName = name,
            mediaType = type,
        )
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

// The staged files, shown above the editor once any are attached. The Attach action itself is the
// paperclip in the app bar, so this is just the list; empty means nothing is drawn.
@Composable
internal fun ComposerAttachmentList(
    attachments: List<PickedComposerFile>,
    onRemove: (PickedComposerFile) -> Unit,
) {
    if (attachments.isEmpty()) {
        return
    }
    val ctx = LocalContext.current
    Column(modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
        Text(
            text = L10n.attachments_title(ctx),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        attachments.forEach { attachment ->
            Row(modifier = Modifier.fillMaxWidth()) {
                Text(
                    text = attachment.fileName,
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.bodySmall,
                )
                TextButton(onClick = { onRemove(attachment) }) {
                    Text(L10n.action_remove(ctx))
                }
            }
        }
    }
}
