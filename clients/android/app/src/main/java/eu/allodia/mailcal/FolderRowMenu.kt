// The folder drawer's row menu (docs/folder-pane.md, rule 23): a long press opens it, and each
// item is also a named accessibility action on the row, so TalkBack reaches it without the gesture.
package eu.allodia.mailcal

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics

// Wraps one drawer row with its menu. `row` receives the modifier that carries the long press and
// the accessibility actions; with no items it gets a plain one, and there is no menu at all.
@Composable
internal fun FolderRowWithMenu(
    items: List<FolderMenuItem>,
    onPick: (FolderMenuItem) -> Unit,
    row: @Composable (Modifier) -> Unit,
) {
    if (items.isEmpty()) {
        row(Modifier)
        return
    }
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    Box {
        row(
            Modifier
                .onLongPress { open = true }
                .semantics {
                    customActions = items.map { item ->
                        CustomAccessibilityAction(menuLabel(item, ctx)) {
                            onPick(item)
                            true
                        }
                    }
                },
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            items.forEach { item ->
                DropdownMenuItem(
                    text = { Text(menuLabel(item, ctx)) },
                    onClick = {
                        open = false
                        onPick(item)
                    },
                )
            }
        }
    }
}

// A long press that leaves the row's own click alone: it watches on the initial pass, before the
// row's clickable, and only once the press has outlasted the long-press timeout does it take the
// rest of the gesture, so the release does not also open the folder. A drawer item takes a single
// `onClick`, so there is no `combinedClickable` to put this on.
private fun Modifier.onLongPress(action: () -> Unit): Modifier =
    pointerInput(Unit) {
        awaitEachGesture {
            val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
            val released = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                while (true) {
                    val change = awaitPointerEvent(PointerEventPass.Initial).changes
                        .firstOrNull { it.id == down.id } ?: break
                    val moved = (change.position - down.position).getDistance() >
                        viewConfiguration.touchSlop
                    if (!change.pressed || moved) break
                }
            }
            if (released == null) {
                action()
                while (true) {
                    val event = awaitPointerEvent(PointerEventPass.Initial)
                    event.changes.forEach { it.consume() }
                    if (event.changes.none { it.pressed }) break
                }
            }
        }
    }
