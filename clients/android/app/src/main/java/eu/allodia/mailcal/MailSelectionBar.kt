// The contextual bar over the message list while rows are selected: the count, a way out, and the
// core's command bar (docs/command-bar.md, rule 9). It replaces the account switcher rather than
// stacking under it, which is the Material pattern for a selection mode and keeps the list where it
// was.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.Command
import uniffi.mailcal_bindings.CommandBar
import uniffi.mailcal_bindings.CommandItem
import uniffi.mailcal_bindings.RowAction
import uniffi.mailcal_bindings.SelectedRow
import uniffi.mailcal_bindings.SnapshotRow

/** How many of the core's actions stand on the bar as icons; the rest are in its overflow. */
private const val BAR_ICONS = 2

/**
 * The selection mode's top bar.
 *
 * The core's first actions, Delete and Archive, are the two icons, because they are what a mailbox
 * is worked through with; the rest follow in the overflow in the core's order, then Select all.
 * Each item runs as the same item in a row menu over the selection does, through [onPick], and Move
 * asks for its folder here, as a row's menu does.
 */
@Composable
internal fun MailSelectionBar(
    selection: MailSelectionState,
    rows: List<SnapshotRow>,
    bar: CommandBar,
    filing: MessageFiling?,
    onPick: (RowPick) -> Unit,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    var overflow by remember { mutableStateOf(false) }
    var moving by remember { mutableStateOf<Pair<List<SelectedRow>, List<MoveTarget>>?>(null) }
    val selected = selection.selectedRows()
    val account = selected.map { it.account() }.distinct().singleOrNull()
    val targets = account?.let { filing?.targets?.invoke(it) }.orEmpty()
    val items = bar.sections.flatMap { it.items }
    fun available(item: CommandItem): Boolean =
        item.enabled && (item.command != Command.Act(RowAction.MOVE_TO_FOLDER) || targets.any { it.enabled })
    fun run(item: CommandItem) {
        val pick = item.menuItem()?.let { rowPick(it, selected) } ?: return
        if (pick is RowPick.Move) moving = pick.rows to targets else onPick(pick)
    }
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
        items.take(BAR_ICONS).forEach { item ->
            val action = (item.command as? Command.Act)?.action ?: return@forEach
            IconButton(onClick = { run(item) }, enabled = available(item)) {
                Icon(
                    painter = painterResource(action.icon()),
                    contentDescription = action.label(ctx),
                    tint = if (item.confirm && item.enabled) {
                        MaterialTheme.colorScheme.error
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                )
            }
        }
        IconButton(onClick = { overflow = true }) {
            Icon(
                painter = painterResource(R.drawable.ic_more_vert),
                contentDescription = L10n.a11y_more_actions(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        DropdownMenu(expanded = overflow, onDismissRequest = { overflow = false }) {
            items.drop(BAR_ICONS).forEach { item ->
                val action = (item.command as? Command.Act)?.action ?: return@forEach
                DropdownMenuItem(
                    text = {
                        Text(
                            action.label(ctx),
                            color = if (item.confirm && item.enabled) MaterialTheme.colorScheme.error else Color.Unspecified,
                        )
                    },
                    enabled = available(item),
                    onClick = {
                        overflow = false
                        run(item)
                    },
                )
            }
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(L10n.action_select_all(ctx)) },
                onClick = {
                    overflow = false
                    selection.selectAll(rows)
                },
            )
        }
    }
    moving?.let { (moved, offered) ->
        MoveTargetDialog(
            title = L10n.message_move_title(ctx),
            targets = offered,
            onPick = { target ->
                val key = target.key
                if (key != null && account != null) filing?.move?.invoke(moved, account, key)
            },
            onClose = { moving = null },
        )
    }
}

/** The glyph a bar action carries as an icon. */
private fun RowAction.icon(): Int = when (this) {
    RowAction.DELETE, RowAction.PERMANENTLY_DELETE -> R.drawable.ic_delete
    RowAction.ARCHIVE -> R.drawable.ic_archive
    else -> R.drawable.ic_folder
}

private fun SelectedRow.account(): String = when (this) {
    is SelectedRow.Message -> account
    is SelectedRow.Thread -> account
}
