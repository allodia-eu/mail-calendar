// The card a drafted reply brings (docs/ai.md, "Where they show"): what the message asks and what
// is left to do, under the address fields and above the text. It is Compose beside the editor, so
// it is never part of the mail. Send asks once while an item is open, and never blocks. The state is
// DraftChecklist (ComposerDraftChecklist.kt).
package eu.allodia.mailcal

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.collapse
import androidx.compose.ui.semantics.expand
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import uniffi.mailcal_bindings.DraftTaskKind

@Composable
internal fun ComposerDraftCard(control: ReplyDraftControl) {
    val checklist = control.checklist
    if (checklist.isEmpty) return
    // About once a second while a fill-in item is open; the card's effect, so it ends with the
    // composer.
    val following = checklist.following
    LaunchedEffect(following) {
        while (following != null && checklist.awaitsPlaceholders) {
            delay(1_000)
            control.readPlaceholders()
        }
    }
    val ctx = LocalContext.current
    val onPhone = LocalConfiguration.current.smallestScreenWidthDp < 600
    val expanded = !checklist.isCollapsed(onPhone)
    val toggle = { checklist.collapsedChoice = expanded }
    Surface(
        modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
    ) {
        Column(modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp)) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = 40.dp)
                    .clickable(onClick = toggle)
                    .semantics {
                        heading()
                        if (expanded) collapse { toggle(); true } else expand { toggle(); true }
                    },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    if (checklist.summary.isEmpty()) {
                        L10n.composer_checklist_heading(ctx)
                    } else {
                        L10n.composer_draft_summary_heading(ctx)
                    },
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f),
                )
                Icon(
                    painterResource(if (expanded) R.drawable.ic_keyboard_arrow_up else R.drawable.ic_keyboard_arrow_down),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            AnimatedVisibility(visible = expanded) {
                Column(
                    modifier = Modifier.padding(bottom = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    if (checklist.summary.isNotEmpty()) {
                        Text(checklist.summary, style = MaterialTheme.typography.bodyMedium)
                        if (checklist.items.isNotEmpty()) {
                            Text(
                                L10n.composer_checklist_heading(ctx),
                                style = MaterialTheme.typography.titleSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(top = 8.dp).semantics { heading() },
                            )
                        }
                    }
                    checklist.items.forEach { TaskRow(it) { checklist.toggle(it.id) } }
                }
            }
        }
    }
}

// A fill-in item ticks itself as the reply changes, so only the others toggle.
@Composable
private fun TaskRow(item: DraftChecklist.Item, onToggle: () -> Unit) {
    val ctx = LocalContext.current
    val byHand = item.kind != DraftTaskKind.FILL_IN
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 40.dp)
            .then(
                if (byHand) {
                    Modifier.toggleable(value = item.ticked, role = Role.Checkbox, onValueChange = { onToggle() })
                } else {
                    Modifier.semantics(mergeDescendants = true) { toggleableState = ToggleableState(item.ticked) }
                },
            ),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Checkbox(checked = item.ticked, onCheckedChange = null)
        Icon(
            painterResource(
                when (item.kind) {
                    DraftTaskKind.FILL_IN -> R.drawable.ic_text_fields
                    DraftTaskKind.ATTACH -> R.drawable.ic_attachment
                    DraftTaskKind.DO -> R.drawable.ic_task_alt
                },
            ),
            contentDescription = when (item.kind) {
                DraftTaskKind.FILL_IN -> null
                DraftTaskKind.ATTACH -> L10n.a11y_task_attach(ctx)
                DraftTaskKind.DO -> L10n.a11y_task_do(ctx)
            },
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(18.dp),
        )
        Text(
            item.title(ctx),
            style = MaterialTheme.typography.bodyMedium.copy(
                textDecoration = if (item.ticked) TextDecoration.LineThrough else null,
            ),
            color = if (item.ticked) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
        )
    }
}

// Send with items open: asked once per composer, and back or a tap outside is keeping on editing.
@Composable
internal fun ComposerSendOpenQuestion(control: ReplyDraftControl) {
    if (!control.confirmingSend) return
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = control::keepEditing,
        title = { Text(L10n.composer_send_open_title(ctx)) },
        text = { Text(L10n.composer_send_open_message(ctx, control.checklist.openCount)) },
        confirmButton = {
            TextButton(onClick = control::sendAnyway) { Text(L10n.composer_send_anyway(ctx)) }
        },
        dismissButton = {
            TextButton(onClick = control::keepEditing) { Text(L10n.composer_keep_editing(ctx)) }
        },
    )
}
