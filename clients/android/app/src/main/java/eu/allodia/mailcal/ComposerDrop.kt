// Files dragged onto the composer, and the question a picture raises.
//
// A drop is handled NATIVELY, not by the page. The editor bundle refuses `drop`, because web code
// only ever sees a `File` with no path: it could neither hand the bytes to Rust for a streamed send
// nor put a removable row in the attachment list. The host resolves the drop to a staged file, so
// both work, and the page is handed a picture only when the user asks for one.
//
// A picture raises the question the other formats do not: it can be shown where the user is typing
// (an inline `cid:` part, what Outlook does) or sent as a file to download. Everything else is
// simply attached. The question is asked once for the whole drop.
package eu.allodia.mailcal

import android.app.Activity
import android.content.ClipDescription
import android.os.Handler
import android.os.Looper
import android.webkit.WebView
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.mimeTypes
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import androidx.compose.ui.platform.LocalContext
import java.io.File
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.composerImageDataUrl

// What a drop turns into, before anything is shown or attached.
internal data class DroppedComposerFiles(
    // Files with nothing to decide: attached straight away.
    val attach: List<PickedComposerFile> = emptyList(),
    // Pictures, which the user is asked about.
    val pictures: List<PickedComposerFile> = emptyList(),
)

internal fun sortDroppedFiles(files: List<PickedComposerFile>): DroppedComposerFiles {
    val (pictures, attach) = files.partition { it.mediaType.startsWith("image/") }
    return DroppedComposerFiles(attach = attach, pictures = pictures)
}

// Whether a drag carries something the composer can take.
//
// Plain text and HTML are the editor's own business (they land in the body through the normal
// paste/drop machinery), so a drag of nothing else is left alone rather than turned into an
// attachment named after a text snippet. Anything else is a file worth taking.
internal fun acceptsComposerDrop(mimeTypes: Collection<String>): Boolean =
    mimeTypes.any {
        it != ClipDescription.MIMETYPE_TEXT_PLAIN &&
            it != ClipDescription.MIMETYPE_TEXT_HTML &&
            it != ClipDescription.MIMETYPE_TEXT_INTENT
    }

// The composer's drop target. Staging copies each dropped document into the app cache off the main
// thread, exactly as the Attach picker does, so `onDropped` always arrives with real paths Rust can
// read.
@Composable
internal fun Modifier.composerDropTarget(onDropped: (DroppedComposerFiles) -> Unit): Modifier {
    val ctx = LocalContext.current
    val target = remember(ctx) {
        object : DragAndDropTarget {
            override fun onDrop(event: DragAndDropEvent): Boolean {
                val android = event.toAndroidDragEvent()
                // Without this the URIs belong to the other app and every read fails; it grants
                // this activity access for as long as it lives, which outlasts the copy below.
                (ctx as? Activity)?.requestDragAndDropPermissions(android)
                val clip = android.clipData ?: return false
                val uris = (0 until clip.itemCount).mapNotNull { clip.getItemAt(it).uri }
                if (uris.isEmpty()) {
                    return false
                }
                thread(name = "mailcal-stage-dropped") {
                    val staged = uris.mapNotNull { uri -> stageComposerFile(ctx, uri) }
                    Handler(Looper.getMainLooper()).post { onDropped(sortDroppedFiles(staged)) }
                }
                return true
            }
        }
    }
    return dragAndDropTarget(
        shouldStartDragAndDrop = { acceptsComposerDrop(it.mimeTypes()) },
        target = target,
    )
}

// The dropped-picture question and what each answer does. Nothing is drawn while `pictures` is
// empty, which is the state the composer sits in except in the moment between a drop and its
// answer.
//
// Cancelling deletes the staged copies: they were made for this drop and nothing else refers to
// them.
@Composable
internal fun ComposerDroppedPictureQuestion(
    pictures: List<PickedComposerFile>,
    webView: WebView?,
    onAttach: (List<PickedComposerFile>) -> Unit,
    onUnreadable: () -> Unit,
    onAnswered: () -> Unit,
) {
    if (pictures.isEmpty()) {
        return
    }
    ComposerImageChoiceDialog(
        onShowInMessage = {
            onAnswered()
            showPicturesInMessage(webView, pictures) { unreadable ->
                if (unreadable.isNotEmpty()) {
                    onUnreadable()
                    onAttach(unreadable)
                }
            }
        },
        onSendAsFile = {
            onAnswered()
            onAttach(pictures)
        },
        onCancel = {
            onAnswered()
            pictures.forEach { File(it.path).delete() }
        },
    )
}

// The question a dropped picture raises, in the words a reader of the message would use: nothing
// about inline parts, attachments or MIME.
@Composable
private fun ComposerImageChoiceDialog(
    onShowInMessage: () -> Unit,
    onSendAsFile: () -> Unit,
    onCancel: () -> Unit,
) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onCancel,
        title = { Text(L10n.compose_image_drop_title(ctx)) },
        text = { Text(L10n.compose_image_drop_body(ctx)) },
        confirmButton = {
            TextButton(onClick = onShowInMessage) {
                Text(L10n.compose_image_drop_inline(ctx))
            }
        },
        dismissButton = {
            TextButton(onClick = onSendAsFile) { Text(L10n.compose_image_drop_attach(ctx)) }
        },
    )
}

// Reads each picture and hands it to the shared editor, which inserts it at the caret and records
// the inline attachment the core turns into a `cid:` part on send.
//
// The read is the core's: it sniffs the bytes rather than trusting the name, and holds the size
// cap, so all four clients answer "can this be shown?" the same way. It runs off the main thread
// because it reads whole files.
//
// A picture the core cannot read as one is handed back through `onUnreadable` to be attached
// instead: the user asked for it to be in the message, and losing it silently is the worse answer.
internal fun showPicturesInMessage(
    webView: WebView?,
    pictures: List<PickedComposerFile>,
    onUnreadable: (List<PickedComposerFile>) -> Unit,
) {
    if (pictures.isEmpty()) {
        return
    }
    thread(name = "mailcal-read-pictures") {
        val unreadable = mutableListOf<PickedComposerFile>()
        val readable = mutableListOf<Pair<PickedComposerFile, String>>()
        for (picture in pictures) {
            try {
                readable += picture to composerImageDataUrl(picture.path)
            } catch (_: MailcalException) {
                unreadable += picture
            }
        }
        Handler(Looper.getMainLooper()).post {
            for ((picture, dataUrl) in readable) {
                webView?.insertComposerImage(dataUrl, picture.fileName)
                // The bytes live in the editor document now, so the staged copy is dead weight.
                File(picture.path).delete()
            }
            onUnreadable(unreadable)
        }
    }
}
