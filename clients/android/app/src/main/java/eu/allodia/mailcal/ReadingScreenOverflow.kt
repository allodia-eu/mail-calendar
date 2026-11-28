// The reading screen's overflow menu, last in the top app bar, and the .eml export behind it
// (docs/reading-actions.md); printing is in ReadingScreenPrint.kt.
//
// The file written is the message exactly as it was delivered: the core hands over the raw
// source the engine cached, never a rebuild of what the screen renders.
package eu.allodia.mailcal

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import java.io.File
import java.util.UUID
import kotlin.concurrent.thread
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import uniffi.mailcal_bindings.MessageScheme
import uniffi.mailcal_bindings.messageExportFileName

// The tag the overflow button carries, so a test can find it without depending on the icon.
internal const val READING_OVERFLOW_TAG = "reading-overflow"

/**
 * The menu at the end of the reading screen's top app bar.
 *
 * An icon at every width, like the buttons beside it, but unlike them it is identified by its
 * place rather than by its glyph, so it is always last. In a dark app its first item is the
 * sun/moon choice of page ([scheme] is null in a light app, where there is nothing to switch;
 * docs/reading-dark-mode.md), the bar having no room for a seventh button.
 */
@Composable
internal fun ReadingOverflowMenu(
    subject: String,
    account: String,
    key: String,
    onExportMessage: (account: String, key: String, destinationPath: String) -> Boolean,
    // Null until there is a body to print, which leaves the item shown but disabled.
    onPrint: (() -> Unit)?,
    scheme: MessageScheme? = null,
    onSetDarkMessages: (Boolean) -> Unit = {},
) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    val export = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("message/rfc822"),
    ) { uri ->
        if (uri != null) {
            exportMessage(ctx, uri, account, key, onExportMessage)
        }
    }
    IconButton(onClick = { open = true }, modifier = Modifier.testTag(READING_OVERFLOW_TAG)) {
        Icon(
            painter = painterResource(R.drawable.ic_more_vert),
            contentDescription = L10n.a11y_more_actions(ctx),
        )
    }
    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
        if (scheme != null) {
            // Its words say what pressing it does: back to light on a dark message, and the reverse.
            val dark = scheme == MessageScheme.DARK
            DropdownMenuItem(
                text = {
                    Text(
                        if (dark) {
                            L10n.reading_show_light_background(ctx)
                        } else {
                            L10n.reading_show_dark_background(ctx)
                        },
                    )
                },
                // A sun on a dark message, a moon on a light one (docs/icons.md).
                leadingIcon = {
                    Icon(
                        painter = painterResource(if (dark) R.drawable.ic_light_mode else R.drawable.ic_dark_mode),
                        contentDescription = null,
                    )
                },
                onClick = {
                    open = false
                    onSetDarkMessages(!dark)
                },
            )
            HorizontalDivider()
        }
        DropdownMenuItem(
            text = { Text(L10n.action_save_as_eml(ctx)) },
            onClick = {
                open = false
                export.launch(messageExportFileName(subject))
            },
        )
        DropdownMenuItem(
            text = { Text(L10n.action_print(ctx)) },
            enabled = onPrint != null,
            onClick = {
                open = false
                onPrint?.invoke()
            },
        )
    }
}

/**
 * Writes the open message to `uri`, then says whether it worked.
 *
 * Off the main thread, like the attachment save it mirrors: the raw source may not be cached,
 * and fetching it would otherwise freeze the screen. The core writes to a cache file we own and
 * the bytes are copied into the picked document, because a `content://` URI is not a path the
 * core can write to.
 */
internal fun exportMessage(
    ctx: Context,
    uri: Uri,
    account: String,
    key: String,
    onExportMessage: (account: String, key: String, destinationPath: String) -> Boolean,
) {
    thread(name = "mailcal-export-message") {
        val temp = File(ctx.cacheDir, "exported-messages/${UUID.randomUUID()}.eml")
        temp.parentFile?.mkdirs()
        val written = onExportMessage(account, key, temp.absolutePath)
        val copied = if (written) {
            try {
                ctx.contentResolver.openOutputStream(uri)?.use { output ->
                    temp.inputStream().use { input -> input.copyTo(output) }
                } != null
            } catch (_: Exception) {
                false
            } finally {
                temp.delete()
            }
        } else {
            temp.delete()
            false
        }
        Handler(Looper.getMainLooper()).post {
            Toast.makeText(
                ctx,
                if (copied) L10n.message_saved(ctx) else L10n.message_save_failed(ctx),
                Toast.LENGTH_SHORT,
            ).show()
        }
    }
}
