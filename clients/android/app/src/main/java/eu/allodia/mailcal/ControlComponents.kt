// The controls a page draws from the core's ControlState (docs/waiting.md): a switch row, a button,
// a dropdown and a confirmation. Each is held while it cannot be used and shows a spinner while
// work it started runs, so every page waits the same way.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.focus.onFocusChanged
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ControlState

/** The spinner beside a control whose work is running. */
@Composable
private fun Running(control: ControlState, tag: String) {
    if (control.running) {
        CircularProgressIndicator(modifier = Modifier.size(18.dp).testTag("$tag-running"))
        Spacer(modifier = Modifier.width(8.dp))
    }
}

/**
 * A switch row: the whole row is the switch, so it is one target and one announcement. [onChange]
 * is told what the person switched it to.
 */
@Composable
internal fun ControlSwitchRow(
    title: String,
    note: String?,
    on: Boolean,
    control: ControlState,
    tag: String,
    onChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(value = on, enabled = control.enabled, role = Role.Switch, onValueChange = onChange)
            .padding(vertical = 4.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            note?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Spacer(modifier = Modifier.width(8.dp))
        Running(control, tag)
        Switch(checked = on, onCheckedChange = null, enabled = control.enabled)
    }
}

/** A button. A destructive one is drawn in the error colour. */
@Composable
internal fun ControlButton(
    label: String,
    control: ControlState,
    tag: String,
    modifier: Modifier = Modifier,
    destructive: Boolean = false,
    onClick: () -> Unit,
) {
    val colors = if (destructive) {
        ButtonDefaults.buttonColors(
            containerColor = MaterialTheme.colorScheme.error,
            contentColor = MaterialTheme.colorScheme.onError,
        )
    } else {
        ButtonDefaults.buttonColors()
    }
    Button(
        onClick = onClick,
        enabled = control.enabled,
        colors = colors,
        modifier = modifier.testTag(tag),
    ) {
        Running(control, tag)
        Text(label)
    }
}

/** A text button, for an action beside a row rather than the row's own. */
@Composable
internal fun ControlTextButton(
    label: String,
    control: ControlState,
    tag: String,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    TextButton(onClick = onClick, enabled = control.enabled, modifier = modifier.testTag(tag)) {
        Running(control, tag)
        Text(label)
    }
}

/**
 * A text field under its title, which tells [onCommit] what was typed on Done and when it loses
 * focus, and only when it differs from what it last told, starting from [text]: never on every
 * keystroke, which would store a half-typed value and redraw the page under the cursor, and never
 * twice for one edit, as Done and then the focus leaving would. The focus leaving includes another
 * change holding the field; the core runs what was typed once that change ends.
 */
@Composable
internal fun ControlTextField(
    title: String,
    text: String,
    control: ControlState,
    tag: String,
    onCommit: (String) -> Unit,
) {
    // Seeded again whenever the core draws another value, so a field never carries what an older
    // page said.
    var draft by remember(text) { mutableStateOf(text) }
    var sent by remember(text) { mutableStateOf(text) }
    val commit = {
        if (draft != sent) {
            sent = draft
            onCommit(draft)
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            value = draft,
            onValueChange = { draft = it },
            singleLine = true,
            enabled = control.enabled,
            label = { Text(title) },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { commit() }),
            modifier = Modifier
                .weight(1f)
                .testTag(tag)
                .onFocusChanged { focus -> if (!focus.isFocused) commit() },
        )
        Running(control, tag)
    }
}

/**
 * A dropdown under its title, and [note] when there is one: [labels] listed, [selected] shown.
 * [onPick] is told the position the person picked, and only when it differs from [selected].
 */
@Composable
internal fun ControlDropdown(
    title: String,
    labels: List<String>,
    selected: Int,
    control: ControlState,
    tag: String,
    note: String? = null,
    onPick: (Int) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Text(title, style = MaterialTheme.typography.labelLarge)
    note?.let {
        Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box {
            TextButton(
                onClick = { expanded = true },
                enabled = control.enabled,
                modifier = Modifier.testTag(tag),
            ) {
                Text(labels.getOrElse(selected) { "" }, maxLines = 1)
                Icon(
                    painter = painterResource(R.drawable.ic_arrow_drop_down),
                    contentDescription = title,
                )
            }
            DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                labels.forEachIndexed { index, label ->
                    DropdownMenuItem(
                        text = { Text(label) },
                        onClick = {
                            expanded = false
                            if (index != selected) onPick(index)
                        },
                    )
                }
            }
        }
        Running(control, tag)
    }
}

/**
 * Asks before a change that deletes something: [action] does it, Cancel does not. [onAnswer] is
 * told which, however the question goes away.
 */
@Composable
internal fun ConfirmDialog(
    title: String,
    message: String,
    action: String,
    tag: String,
    onAnswer: (Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = { onAnswer(false) },
        title = { Text(title) },
        text = { Text(message, modifier = Modifier.testTag("$tag-message")) },
        confirmButton = {
            TextButton(
                onClick = { onAnswer(true) },
                modifier = Modifier.testTag("$tag-confirm"),
            ) { Text(action) }
        },
        dismissButton = {
            TextButton(onClick = { onAnswer(false) }) { Text(L10n.action_cancel(ctx)) }
        },
    )
}
