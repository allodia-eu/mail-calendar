// The contextual bar over the message list while rows are selected: the count, a way out, and the
// actions docs/list-selection.md gives it. It replaces the account switcher rather than stacking
// under it, which is the Material pattern for a selection mode and keeps the list where it was.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.SnapshotRow

/**
 * The selection mode's top bar.
 *
 * Archive and Trash are the two icons, because they are what a mailbox is worked through with; the
 * rest (read/unread, flag/unflag, select all, delete permanently) live in the overflow the row
 * already has room for. The read and flag entries are single toggles whose wording comes from what
 * is selected, so the offered action is always the one that changes something.
 */
@Composable
internal fun MailSelectionBar(
    selection: MailSelectionState,
    rows: List<SnapshotRow>,
    onAct: (BulkAction) -> Unit,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    var overflow by remember { mutableStateOf(false) }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 4.dp, vertical = 8.dp)
            .testTag("selection-bar"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = selection::clear) {
            Icon(
                painter = painterResource(R.drawable.ic_close),
                contentDescription = L10n.action_clear_selection(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            text = L10n.selection_count(ctx, selection.count),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier
                .weight(1f)
                .testTag("selection-count"),
        )
        IconButton(onClick = { onAct(BulkAction.ARCHIVE) }) {
            Icon(
                painter = painterResource(R.drawable.ic_archive),
                contentDescription = L10n.action_archive(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        IconButton(onClick = { onAct(BulkAction.DELETE) }) {
            Icon(
                painter = painterResource(R.drawable.ic_delete),
                contentDescription = L10n.action_move_to_trash(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        IconButton(onClick = { overflow = true }) {
            Icon(
                painter = painterResource(R.drawable.ic_more_vert),
                contentDescription = L10n.a11y_more_actions(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        DropdownMenu(expanded = overflow, onDismissRequest = { overflow = false }) {
            val read = selection.readAction(rows)
            val flag = selection.flagAction(rows)
            DropdownMenuItem(
                text = {
                    Text(
                        if (read == BulkAction.MARK_READ) {
                            L10n.action_mark_read(ctx)
                        } else {
                            L10n.action_mark_unread(ctx)
                        },
                    )
                },
                onClick = {
                    overflow = false
                    onAct(read)
                },
            )
            DropdownMenuItem(
                text = {
                    Text(
                        if (flag == BulkAction.FLAG) L10n.action_flag(ctx) else L10n.action_unflag(ctx),
                    )
                },
                onClick = {
                    overflow = false
                    onAct(flag)
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_select_all(ctx)) },
                onClick = {
                    overflow = false
                    selection.selectAll(rows)
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_delete_permanently(ctx)) },
                onClick = {
                    overflow = false
                    onAct(BulkAction.PERMANENTLY_DELETE)
                },
            )
        }
    }
}
