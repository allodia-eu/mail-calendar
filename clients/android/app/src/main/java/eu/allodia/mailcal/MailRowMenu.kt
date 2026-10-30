// The ⋮ menu every list row carries, a message's and a conversation's alike, drawn as the core
// answers it (docs/row-actions.md): asked as it opens, its sections in order with a divider
// between them, each item greyed out where it does not respond. What picking an item does is
// RowMenuPresentation.kt's.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import uniffi.mailcal_bindings.RowAction
import uniffi.mailcal_bindings.RowMenu
import uniffi.mailcal_bindings.SelectedRow

/**
 * What a row's menu reads when it opens.
 *
 * [rowsFor] names the rows a menu opened on a row acts on: the selection when that row is one of
 * the selected rows (docs/list-selection.md, rule 12), and the row alone otherwise.
 */
internal class RowMenus(
    val menu: (List<SelectedRow>) -> RowMenu,
    val rowsFor: (SelectedRow) -> List<SelectedRow>,
    val filing: MessageFiling?,
    val onPick: (RowPick) -> Unit,
) {
    companion object {
        /** A menu holding nothing, for a list drawn without a core: a test or a screenshot. */
        val NONE = RowMenus(
            menu = { RowMenu(emptyList()) },
            rowsFor = { listOf(it) },
            filing = null,
            onPick = {},
        )
    }
}

@Composable
internal fun RowMenuButton(row: SelectedRow, account: String, menus: RowMenus) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf<Pair<List<SelectedRow>, RowMenu>?>(null) }
    // The Move to folder… list while its dialog is open, and the rows it moves.
    var filing by remember { mutableStateOf<Pair<List<SelectedRow>, List<MoveTarget>>?>(null) }
    Box {
        IconButton(
            onClick = {
                val rows = menus.rowsFor(row)
                open = rows to menus.menu(rows)
            },
            modifier = Modifier.testTag("row-menu"),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_more_vert),
                contentDescription = L10n.a11y_more_actions(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        DropdownMenu(expanded = open != null, onDismissRequest = { open = null }) {
            val (rows, menu) = open ?: return@DropdownMenu
            // Where a move could go, in the rows' one account; none leaves the item disabled.
            val targets = remember(rows) { menus.filing?.targets?.invoke(account).orEmpty() }
            menu.sections.forEachIndexed { index, section ->
                if (index > 0) HorizontalDivider()
                section.items.forEach { item ->
                    val pick = rowPick(item, rows)
                    val movable = item.action != RowAction.MOVE_TO_FOLDER ||
                        targets.any { it.enabled }
                    val enabled = item.enabled && movable && pick != null
                    DropdownMenuItem(
                        text = {
                            Text(
                                text = item.action.label(ctx),
                                color = if (item.action == RowAction.PERMANENTLY_DELETE && enabled) {
                                    MaterialTheme.colorScheme.error
                                } else {
                                    androidx.compose.ui.graphics.Color.Unspecified
                                },
                            )
                        },
                        enabled = enabled,
                        onClick = {
                            open = null
                            when (pick) {
                                is RowPick.Move -> filing = pick.rows to targets
                                null -> Unit
                                else -> menus.onPick(pick)
                            }
                        },
                    )
                }
            }
        }
    }
    filing?.let { (rows, targets) ->
        MoveTargetDialog(
            title = L10n.message_move_title(ctx),
            targets = targets,
            onPick = { target -> target.key?.let { menus.filing?.move?.invoke(rows, account, it) } },
            onClose = { filing = null },
        )
    }
}
