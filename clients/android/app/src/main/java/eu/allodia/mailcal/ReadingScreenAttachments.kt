// The attachment list on the reading screen, split out of ReadingScreen.kt: the cap on how much
// screen it may take, decoding + handing an attachment to the OS default viewer via a
// FileProvider content URI, and saving one to a picked location. Never rendered or executed
// in-app; opening it always goes through the OS's own file handling.
package eu.allodia.mailcal

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.text.format.Formatter
import android.widget.Toast
import java.io.File
import java.util.UUID
import kotlin.concurrent.thread
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import uniffi.mailcal_bindings.AttachmentRow

// How much of the screen the attachment list may take before it starts scrolling inside itself.
//
// Roughly four rows. The list sits ABOVE the message in a column that does not scroll, so this is
// the line between "this message has attachments" and "this message is unreadable": a quote pack
// or a scanned bundle really does reach twenty files, and twenty rows push the body clean off the
// bottom with no way to reach either it or the last few files.
internal val ATTACHMENT_LIST_CAP = 168.dp

// The tag the list's scrolling half carries, so a test can measure what the cap actually did.
internal const val ATTACHMENT_ROWS_TAG = "attachment-rows"

@Composable
internal fun AttachmentList(
    attachments: List<AttachmentRow>,
    onSave: (AttachmentRow) -> Unit,
    onOpen: (AttachmentRow) -> Unit,
) {
    val ctx = LocalContext.current
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = L10n.attachments_title(ctx),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        // `heightIn(max=)` wraps the rows until they reach the cap and stops there, so two
        // attachments take two rows' worth rather than reserving the whole cap.
        Column(
            modifier = Modifier
                .testTag(ATTACHMENT_ROWS_TAG)
                .heightIn(max = ATTACHMENT_LIST_CAP)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
        attachments.forEach { attachment ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = attachment.fileName,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = "${attachment.mediaType} · ${formatBytes(ctx, attachment.size)}",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(onClick = { onOpen(attachment) }) {
                    Text(L10n.action_open(ctx))
                }
                TextButton(onClick = { onSave(attachment) }) {
                    Text(L10n.action_save(ctx))
                }
            }
        }
        }
    }
}

// Decodes one attachment to an app-private cache file off the main thread, then opens it in
// the OS default handler (via a FileProvider content URI). Opening through the OS, never our
// own WebView, means the file passes the OS's own scanning and we never render/execute it.
internal fun openAttachment(
    ctx: Context,
    account: String,
    key: String,
    attachment: AttachmentRow,
    onSaveAttachment: (account: String, key: String, attachmentId: UInt, destinationPath: String) -> Boolean,
) {
    thread(name = "mailcal-open-attachment") {
        val dir = File(ctx.cacheDir, "opened-attachments").apply { mkdirs() }
        val file = File(dir, "${UUID.randomUUID()}-${safeOpenName(attachment.fileName)}")
        val saved = onSaveAttachment(account, key, attachment.id, file.absolutePath)
        Handler(Looper.getMainLooper()).post {
            val opened = saved && launchViewer(ctx, file, attachment.mediaType)
            if (!opened) {
                Toast.makeText(ctx, L10n.attachment_open_failed(ctx), Toast.LENGTH_SHORT).show()
            }
        }
    }
}

private fun launchViewer(ctx: Context, file: File, mediaType: String): Boolean = try {
    val uri = FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", file)
    val intent = Intent(Intent.ACTION_VIEW).apply {
        setDataAndType(uri, mediaType.ifEmpty { "*/*" })
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    ctx.startActivity(intent)
    true
} catch (_: ActivityNotFoundException) {
    false
} catch (_: Exception) {
    false
}

private fun safeOpenName(fileName: String): String {
    val cleaned = fileName.map { ch ->
        if (ch.isISOControl() || ch in setOf('/', '\\', ':', '*', '?', '"', '<', '>', '|')) '_' else ch
    }.joinToString("").trim('.', ' ', '_')
    return cleaned.ifEmpty { "attachment" }
}

private fun formatBytes(ctx: Context, bytes: ULong): String {
    val capped = bytes.coerceAtMost(Long.MAX_VALUE.toULong()).toLong()
    return Formatter.formatFileSize(ctx, capped)
}
