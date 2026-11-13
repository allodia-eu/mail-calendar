// The files attached to a composer's message, drawn as tiles from the core's list
// (docs/composer-layout.md, rules 3 and 4).
//
// The core holds the list. The composer draws the core's last answer and changes the list only
// through it: the picker, a drop and a dropped picture sent as a file add, a tile's Remove takes
// its file off, and each redraws from what the call answered. Every save and the send read the same
// list in the core, so the file on screen and the file that goes out cannot differ.
package eu.allodia.mailcal

import android.text.format.Formatter
import android.util.Log
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.ComposerFileTile
import uniffi.mailcal_bindings.ComposerFiles
import uniffi.mailcal_bindings.MailcalApp

private const val TAG = "Mailcal"

/** A composer holding no files, before the core has been asked or where there is none. */
internal val NO_COMPOSER_FILES = ComposerFiles(tiles = emptyList(), changed = false)

/**
 * The core's calls on one composition's files. Each answers with the list as it now stands, or
 * null when the core refused, which leaves what is on screen as it was. An interface so the JVM
 * suite, which loads no native library, can stand one in.
 */
internal interface ComposerFileList {
    fun current(): ComposerFiles?

    fun add(files: List<ComposerFileAttachment>): ComposerFiles?

    fun remove(id: String): ComposerFiles?
}

/** The core's list for `composition`. */
internal fun composerFileList(instance: MailcalApp, composition: String): ComposerFileList =
    object : ComposerFileList {
        override fun current() = asked { instance.composerFiles(composition) }

        override fun add(files: List<ComposerFileAttachment>) =
            asked { instance.addComposerFiles(composition, files) }

        override fun remove(id: String) = asked { instance.removeComposerFile(composition, id) }
    }

private fun asked(call: () -> ComposerFiles): ComposerFiles? = try {
    call()
} catch (e: Exception) {
    Log.w(TAG, "composer files: ${e.javaClass.simpleName}")
    null
}

/** The narrowest a tile is drawn; the row holds as many equal columns of it as fit. */
private val TILE_WIDTH = 160.dp
private val TILE_GAP = 8.dp

/**
 * The tiles, in rows of equal-width columns that wrap and grow downward without a cap. Nothing is
 * drawn for a composer holding no files.
 */
@Composable
internal fun ComposerFileTiles(files: ComposerFiles, onRemove: (ComposerFileTile) -> Unit) {
    if (files.tiles.isEmpty()) {
        return
    }
    BoxWithConstraints(modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
        val columns = ((maxWidth + TILE_GAP) / (TILE_WIDTH + TILE_GAP)).toInt().coerceAtLeast(1)
        val width = (maxWidth - TILE_GAP * (columns - 1)) / columns
        FlowRow(
            modifier = Modifier.fillMaxWidth().testTag("composer-files"),
            horizontalArrangement = Arrangement.spacedBy(TILE_GAP),
            verticalArrangement = Arrangement.spacedBy(TILE_GAP),
        ) {
            files.tiles.forEach { tile ->
                FileTile(tile, Modifier.width(width)) { onRemove(tile) }
            }
        }
    }
}

/**
 * One file: its name, truncated in the middle so the extension stays, its size in the device's
 * own units, and a Remove drawn from the core's state for it.
 */
@Composable
private fun FileTile(tile: ComposerFileTile, modifier: Modifier, onRemove: () -> Unit) {
    val ctx = LocalContext.current
    Surface(
        modifier = modifier.testTag("composer-file"),
        shape = MaterialTheme.shapes.small,
        color = MaterialTheme.colorScheme.surfaceVariant,
    ) {
        Row(
            modifier = Modifier.padding(start = 12.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = tile.fileName,
                    maxLines = 1,
                    overflow = TextOverflow.MiddleEllipsis,
                    style = MaterialTheme.typography.bodySmall,
                )
                // A file that could not be read has no size to give, and the send says so if it
                // still cannot be read then.
                tile.size?.let { bytes ->
                    Text(
                        text = Formatter.formatShortFileSize(ctx, bytes.toLong()),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            ControlTextButton(
                label = L10n.action_remove(ctx),
                control = tile.remove,
                tag = "composer-file-remove",
                onClick = onRemove,
            )
        }
    }
}
