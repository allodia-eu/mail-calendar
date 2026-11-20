// The controls a page draws from the core's ControlState (docs/waiting.md): a switch row, a row
// that opens another page, a button, a dropdown, a segmented choice and a confirmation. Each is
// held while it cannot be used and shows a spinner while work it started runs, so every page waits
// the same way.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.foundation.layout.RowScope
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.ui.text.style.TextAlign
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
import androidx.compose.ui.unit.Dp
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

/**
 * A row that opens another page and starts no work, with a chevron at its end: one target, read as
 * a button named by its title and [note]. Held while [control] is, and never drawn running: what it
 * opens starts nothing.
 */
@Composable
internal fun ControlNavRow(
    title: String,
    note: String,
    control: ControlState,
    tag: String,
    // A glyph in a tonal circle at the start, for a row that stands for a thing of its own.
    icon: Int? = null,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = control.enabled, role = Role.Button, onClick = onClick)
            .padding(vertical = 8.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Box(
                modifier = Modifier
                    .padding(end = 16.dp)
                    .size(40.dp)
                    .background(MaterialTheme.colorScheme.secondaryContainer, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    painter = painterResource(icon),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSecondaryContainer,
                    modifier = Modifier.size(20.dp),
                )
            }
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(
                note,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        Icon(
            painter = painterResource(R.drawable.ic_keyboard_arrow_right),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * A form's fields, held while [control] is, without a spinner: the spinner is the form's send
 * button's. [content] is told whether its fields take input.
 */
@Composable
internal fun HeldFields(control: ControlState, content: @Composable (enabled: Boolean) -> Unit) {
    content(control.enabled)
}

/**
 * A button. A destructive one is drawn in the error colour, and an [outlined] one, for the lesser
 * of two actions, in outline. One that sends a form is also held while the form is not [ready] to
 * send. Its label wraps between words when the button is narrower than it.
 */
@Composable
internal fun ControlButton(
    label: String,
    control: ControlState,
    tag: String,
    modifier: Modifier = Modifier,
    destructive: Boolean = false,
    outlined: Boolean = false,
    ready: Boolean = true,
    onClick: () -> Unit,
) {
    val content: @Composable RowScope.() -> Unit = {
        Running(control, tag)
        Text(label, textAlign = TextAlign.Center)
    }
    if (outlined) {
        OutlinedButton(
            onClick = onClick,
            enabled = control.enabled && ready,
            colors = if (destructive) {
                ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error)
            } else {
                ButtonDefaults.outlinedButtonColors()
            },
            modifier = modifier.testTag(tag),
            content = content,
        )
        return
    }
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
        enabled = control.enabled && ready,
        colors = colors,
        modifier = modifier.testTag(tag),
        content = content,
    )
}

/** A text button, for an action beside a row rather than the row's own, led by [icon] if given. */
@Composable
internal fun ControlTextButton(
    label: String,
    control: ControlState,
    tag: String,
    modifier: Modifier = Modifier,
    icon: Int? = null,
    onClick: () -> Unit,
) {
    TextButton(onClick = onClick, enabled = control.enabled, modifier = modifier.testTag(tag)) {
        Running(control, tag)
        if (icon != null && !control.running) {
            Icon(
                painter = painterResource(icon),
                contentDescription = null,
                modifier = Modifier.padding(end = 8.dp),
            )
        }
        Text(label)
    }
}

/**
 * An icon button, for an action beside a row where a label would take the row's room, or in an app
 * bar's actions ([iconSize] 24dp, as the bar's own icons). [label] is what assistive technology
 * announces; the spinner takes the icon's place while its work runs. A destructive one draws its
 * icon in the error colour.
 */
@Composable
internal fun ControlIconButton(
    icon: Int,
    label: String,
    control: ControlState,
    tag: String,
    modifier: Modifier = Modifier,
    destructive: Boolean = false,
    iconSize: Dp = 20.dp,
    onClick: () -> Unit,
) {
    IconButton(onClick = onClick, enabled = control.enabled, modifier = modifier.testTag(tag)) {
        if (control.running) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp).testTag("$tag-running"))
        } else {
            Icon(
                painter = painterResource(icon),
                contentDescription = label,
                modifier = Modifier.size(iconSize),
                tint = if (destructive && control.enabled) {
                    MaterialTheme.colorScheme.error
                } else {
                    LocalContentColor.current
                },
            )
        }
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
    // More than one line makes room for a paragraph; Done still sends it.
    minLines: Int = 1,
    supporting: String? = null,
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
            singleLine = minLines == 1,
            minLines = minLines,
            enabled = control.enabled,
            label = { Text(title) },
            supportingText = supporting?.let { { Text(it) } },
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
 * A single choice drawn as a segmented row: [labels] side by side, [selected] marked, each segment
 * tagged `"$tag-<position>"`. [onPick] is told the position the person picked, and only when it
 * differs from [selected].
 */
@Composable
internal fun ControlSegmented(
    labels: List<String>,
    selected: Int,
    control: ControlState,
    tag: String,
    onPick: (Int) -> Unit,
) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.testTag(tag)) {
        Running(control, tag)
        SingleChoiceSegmentedButtonRow(modifier = Modifier.weight(1f)) {
            labels.forEachIndexed { index, label ->
                SegmentedButton(
                    selected = index == selected,
                    onClick = { if (index != selected) onPick(index) },
                    enabled = control.enabled,
                    shape = SegmentedButtonDefaults.itemShape(index = index, count = labels.size),
                    modifier = Modifier.testTag("$tag-$index"),
                ) { Text(label, maxLines = 1) }
            }
        }
    }
}

/**
 * A checkbox row: the whole row is the checkbox, one target. [onChange] is told what the person
 * ticked it to; a row whose [control] is held follows something else and cannot be ticked by hand.
 */
@Composable
internal fun ControlCheckRow(
    checked: Boolean,
    control: ControlState,
    tag: String,
    content: @Composable RowScope.() -> Unit,
    onChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .toggleable(value = checked, enabled = control.enabled, role = Role.Checkbox, onValueChange = onChange)
            .padding(vertical = 2.dp)
            .heightIn(min = 48.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = checked, onCheckedChange = null, enabled = control.enabled)
        Spacer(modifier = Modifier.width(4.dp))
        content()
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
