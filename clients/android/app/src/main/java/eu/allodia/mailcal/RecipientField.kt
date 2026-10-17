// One recipient field (To / Cc / Bcc): the finished addresses as pills, the one being typed as
// text, and the autosuggest list under it.
//
// The field's value stays a single comma-separated **String** owned by the composer, because that
// is what the send path parses, the pills are a rendering of it, not a second source of truth. So
// there is no state to keep in sync and no way for what is on screen to disagree with what gets
// sent. `RecipientAutosuggest.kt` owns the string↔(pills, token) split in pure functions the JVM
// suite drives directly.
//
// Two things this fixes over a plain text field:
//
//   * **Each address is visibly one thing.** In a bare field, `a@x.com, b@y.com` is a wall of text
//     where the boundary between two recipients is a comma the user has to find. A wrong or
//     duplicated address is easy to miss, and there is nothing to tap to remove one.
//   * **The caret lands where you would put it.** Accepting a suggestion rewrites the text; with a
//     `String`-valued field Compose keeps whatever offset it had, so the caret sat mid-text and the
//     next keystroke landed inside the address just inserted. This drives a `TextFieldValue` and
//     puts the selection at the end on every programmatic change.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.findRootCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.roundToIntRect
import uniffi.mailcal_bindings.RecipientMatch

/** The narrowest the input in a header row gets before it moves under the pills. */
private val RECIPIENT_INPUT_MIN_WIDTH = 96.dp

/**
 * A recipient field: pills for finished addresses, a text input for the one in progress.
 *
 * [value] is the whole comma-separated field and [onValue] reports it back, the composer's state
 * shape is unchanged, so nothing about submitting a message had to move.
 *
 * [trailing] is an optional control drawn inside the input (the To field's Cc/Bcc chevron).
 *
 * [asHeaderRow] draws the field as one of the composer header's rows ([ComposerHeaderRow]): the
 * label at the start, the pills and the input sharing its lines. Without it the field is an
 * outlined text field with its pills above, which is what a form (the invitee field) is made of.
 *
 * [focusesOnAppear] opens the composer with the caret in this field.
 *
 * [onSubmit] runs on Return, for a host that finishes the typed address itself (the invitee
 * field). Without one, Return and leaving the field finish it here, as a comma would
 * (`docs/contacts.md`).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun RecipientField(
    label: String,
    value: String,
    onValue: (String) -> Unit,
    suggestionsFor: ((String) -> List<RecipientMatch>)?,
    modifier: Modifier = Modifier,
    focusesOnAppear: Boolean = false,
    onSubmit: (() -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    asHeaderRow: Boolean = false,
) {
    val ctx = LocalContext.current
    val committed = committedRecipients(value)
    val token = currentRecipientToken(value)
    val focusRequester = remember { FocusRequester() }
    // Where the field is, and the window it is in, both in window coordinates: what the list's
    // side and height are decided from.
    var fieldBounds by remember { mutableStateOf(IntRect.Zero) }
    var windowBounds by remember { mutableStateOf(IntRect.Zero) }
    // Only the focused field offers suggestions. Moving from To to Cc must close To's list:
    // harmless while it sat in the layout, and covering live content the moment it floats.
    var focused by remember { mutableStateOf(false) }

    // Undo and redo are the field's (`RecipientHistory`), and a value the composer set from
    // outside, a reply's recipients, is where its history starts.
    val history = remember { RecipientHistory(value) }
    if (value != history.current) history.restart(value)
    val change: (String, Boolean) -> Unit = { next, typed ->
        history.record(next, typed)
        onValue(next)
    }
    // Turns the address being typed into a pill; `false` when there is none.
    val finishTyped: () -> Boolean = {
        token.isNotEmpty().also { if (it) change(seededRecipientField(value), false) }
    }

    // The caret. Held as a `TextFieldValue` so a programmatic rewrite can place it; re-seeded
    // whenever the token changes underneath us (a suggestion was accepted, a pill was removed, a
    // reply prefilled the field), always with the selection collapsed at the end.
    //
    // The guard compares TRIMMED, and that is the whole point of it. `currentRecipientToken`
    // trims, so the value that comes back from the parent has lost any space the user just
    // typed: comparing raw, typing "John " round-trips to the token "John", the guard fires,
    // and the field is rewritten without the space. Every space is silently eaten, "John Smith"
    // becomes "JohnSmith", and a name-based autosuggest query can never match. Trimming both
    // sides means only a *real* change of token (a suggestion accepted, a pill removed) resets,
    // and the user's own whitespace and caret placement are left alone.
    var input by remember { mutableStateOf(TextFieldValue(token, TextRange(token.length))) }
    if (input.text.trim() != token) {
        input = TextFieldValue(token, TextRange(token.length))
    }

    val pills: @Composable () -> Unit = {
        committed.forEachIndexed { index, recipient ->
            InputChip(
                selected = false,
                // The caret goes back to the input, where the chord that brings the recipient back
                // is heard.
                onClick = {
                    change(removeRecipient(value, index), false)
                    focusRequester.requestFocus()
                },
                label = {
                    Text(recipient, maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                trailingIcon = {
                    Icon(
                        painter = painterResource(R.drawable.ic_close),
                        // Names the recipient, so the control is distinguishable when a screen
                        // reader reaches the third identical "Remove" button.
                        contentDescription = "$recipient, ${L10n.compose_remove_recipient(ctx)}",
                        modifier = Modifier.size(18.dp),
                    )
                },
                modifier = Modifier.padding(end = 6.dp),
            )
        }
    }
    val onInput: (TextFieldValue) -> Unit = { typed ->
        input = typed
        change(recipientFieldText(committed, typed.text), true)
    }
    val inputModifier = Modifier
        .testTag("recipient-input-$label")
        .onFocusChanged {
            if (focused && !it.isFocused && onSubmit == null) finishTyped()
            focused = it.isFocused
        }
        // Ahead of the input's own handling, which would otherwise take back only the address
        // being typed, and nothing once it is a pill.
        .onPreviewKeyEvent { event ->
            if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
            val step = historyStep(
                event.key.nativeKeyCode,
                ctrl = event.isCtrlPressed,
                shift = event.isShiftPressed,
                alt = event.isAltPressed,
            ) ?: return@onPreviewKeyEvent false
            val back = if (step == HistoryStep.Undo) history.undo() else history.redo()
            if (back != null) onValue(back)
            true
        }
        .focusRequester(focusRequester)
    // The email keyboard, with `@` on its first page (`docs/contacts.md` §4).
    val keyboardOptions = KeyboardOptions(
        keyboardType = KeyboardType.Email,
        capitalization = KeyboardCapitalization.None,
        autoCorrectEnabled = false,
        imeAction = ImeAction.Done,
    )
    // Taken, the keyboard stays up for the next address; left, Done puts it away. In an empty
    // field Done is the platform's, whoever finishes the address.
    val keyboardActions = KeyboardActions(onDone = {
        val taken = when {
            onSubmit == null -> finishTyped()
            token.isEmpty() -> false
            else -> { onSubmit(); true }
        }
        if (!taken) defaultKeyboardAction(ImeAction.Done)
    })

    Column(
        modifier = modifier.fillMaxWidth().onGloballyPositioned { coordinates ->
            fieldBounds = coordinates.boundsInWindow().roundToIntRect()
            windowBounds = coordinates.findRootCoordinates().boundsInWindow().roundToIntRect()
        },
    ) {
        if (asHeaderRow) {
            ComposerHeaderRow(label = label, labelReadAloud = false, trailing = trailing) {
                // The pills and the input share the row's lines, the input taking what the last
                // line leaves, and moving to a line of its own when that is too narrow to type in.
                FlowRow(
                    modifier = Modifier.weight(1f).testTag("recipient-pills"),
                    itemVerticalAlignment = Alignment.CenterVertically,
                ) {
                    pills()
                    HeaderTextField(
                        value = input,
                        onValueChange = onInput,
                        modifier = inputModifier
                            .weight(1f)
                            .widthIn(min = RECIPIENT_INPUT_MIN_WIDTH)
                            // The row's label is hidden from TalkBack, so the input says which
                            // field it is.
                            .semantics { contentDescription = label },
                        keyboardOptions = keyboardOptions,
                        keyboardActions = keyboardActions,
                    )
                }
            }
        } else {
            if (committed.isNotEmpty()) {
                FlowRow(modifier = Modifier.fillMaxWidth().testTag("recipient-pills")) { pills() }
            }
            OutlinedTextField(
                value = input,
                onValueChange = onInput,
                modifier = inputModifier.fillMaxWidth(),
                singleLine = true,
                label = { Text(label) },
                trailingIcon = trailing,
                keyboardOptions = keyboardOptions,
                keyboardActions = keyboardActions,
            )
        }

        RecipientSuggestionList(
            field = value,
            suggestionsFor = suggestionsFor,
            onAccept = { change(it, false) },
            fieldBounds = fieldBounds,
            window = windowBounds,
            focused = focused,
        )
    }

    // The caret opens in the field the user has to start in, an empty To on a new message. A
    // `FocusRequester` cannot be asked before its node is attached, so this runs as an effect
    // rather than inline in composition.
    LaunchedEffect(focusesOnAppear) {
        if (focusesOnAppear) {
            focusRequester.requestFocus()
        }
    }
}
