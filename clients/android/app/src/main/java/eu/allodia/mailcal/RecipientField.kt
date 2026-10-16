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

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
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
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.roundToIntRect
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.RecipientMatch

/**
 * How long the field waits after the last keystroke before asking the core.
 *
 * Long enough that typing a word costs one query rather than one per character, short enough that
 * the dropdown still arrives while the user is looking at the field.
 */
private const val SUGGESTION_DEBOUNCE_MS = 120L

/** The gap between a recipient input and the list beside it. */
private val SUGGESTION_GAP = 4.dp

/** About three suggestions: less room than this under the field, and the list opens above it. */
private val SUGGESTION_MIN_ROOM = 160.dp

/** Which side of the field the list opens on, and the most height it may take there. */
internal data class SuggestionPlacement(val above: Boolean, val maxHeightPx: Int)

/**
 * Where the list goes, from the room the field leaves in the **visible** part of the window: the
 * part the keyboard does not cover (docs/contacts.md §4).
 *
 * Under the field when there is room for about three suggestions there, or at least as much as
 * above it; otherwise above. Either way no taller than that room, so the list never reaches under
 * the keyboard and scrolls instead. The event editor's invitee field is the case that needs the
 * other side: the form scrolls it to just above the keyboard, which leaves no room under it.
 */
internal fun suggestionPlacement(
    field: IntRect,
    visibleTop: Int,
    visibleBottom: Int,
    gapPx: Int,
    minRoomPx: Int,
): SuggestionPlacement {
    val below = visibleBottom - field.bottom - gapPx
    val above = field.top - visibleTop - gapPx
    return if (below >= minRoomPx || below >= above) {
        SuggestionPlacement(above = false, maxHeightPx = below.coerceAtLeast(0))
    } else {
        SuggestionPlacement(above = true, maxHeightPx = above)
    }
}

/**
 * Puts a field's suggestion list on the side [placement] chose, across the window's whole width.
 *
 * The anchor is the field's whole column, because a [Popup] positions inside its PARENT's bounds;
 * the popup adds no height of its own, so that column's bottom edge *is* the bottom of the input.
 * The obvious `Alignment.TopStart` would land the list on top of the input, covering the very text
 * it is completing.
 *
 * Full width, as the platform's own address pickers are: a phone's field is narrower than most
 * addresses, and a list the field's width cuts them short.
 *
 * A named class rather than a lambda because Compose's test framework cannot see where a popup
 * actually lands, a popup owns its own root, so its bounds read as (0, 0) whatever the provider
 * did. The decision is therefore what gets asserted (`RecipientPopupPositionTest`).
 */
internal class BesideTheInput(private val gapPx: Int, private val above: Boolean) : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset = if (above) {
        IntOffset(0, anchorBounds.top - gapPx - popupContentSize.height)
    } else {
        IntOffset(0, anchorBounds.bottom + gapPx)
    }
}

/**
 * A recipient field: pills for finished addresses, a text input for the one in progress.
 *
 * [value] is the whole comma-separated field and [onValue] reports it back, the composer's state
 * shape is unchanged, so nothing about submitting a message had to move.
 *
 * [trailing] is an optional control drawn inside the input (the To field's Cc/Bcc chevron).
 *
 * [focusesOnAppear] opens the composer with the caret in this field.
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
    trailing: (@Composable () -> Unit)? = null,
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

    Column(
        modifier = modifier.fillMaxWidth().onGloballyPositioned { coordinates ->
            fieldBounds = coordinates.boundsInWindow().roundToIntRect()
            windowBounds = coordinates.findRootCoordinates().boundsInWindow().roundToIntRect()
        },
    ) {
        if (committed.isNotEmpty()) {
            FlowRow(modifier = Modifier.fillMaxWidth().testTag("recipient-pills")) {
                committed.forEachIndexed { index, recipient ->
                    InputChip(
                        selected = false,
                        // The caret goes back to the input, where the chord that brings the
                        // recipient back is heard.
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
                                // Names the recipient, so the control is distinguishable when a
                                // screen reader reaches the third identical "Remove" button.
                                contentDescription =
                                    "$recipient, ${L10n.compose_remove_recipient(ctx)}",
                                modifier = Modifier.size(18.dp),
                            )
                        },
                        modifier = Modifier.padding(end = 6.dp),
                    )
                }
            }
        }

        OutlinedTextField(
            value = input,
            onValueChange = { typed ->
                input = typed
                change(recipientFieldText(committed, typed.text), true)
            },
            modifier = Modifier
                .fillMaxWidth()
                .testTag("recipient-input-$label")
                .onFocusChanged { focused = it.isFocused }
                // Ahead of the input's own handling, which would otherwise take back only the
                // address being typed, and nothing once it is a pill.
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
                .focusRequester(focusRequester),
            singleLine = true,
            label = { Text(label) },
            trailingIcon = trailing,
            // The email keyboard, with `@` on its first page (`docs/contacts.md` §4).
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Email,
                capitalization = KeyboardCapitalization.None,
                autoCorrectEnabled = false,
            ),
        )

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

/**
 * The suggestion list floating under one recipient field.
 *
 * Queries on the **current token**, the text after the last comma, because the field holds a
 * list: querying the whole thing matches nothing once a first recipient is entered. Accepting one
 * replaces only that token and appends a separator, which turns it into a pill and empties the
 * input, so the caret ends up at the end with nothing extra to do.
 *
 * The lookup is **not** free and must not run in composition. `recipientSuggestions` blocks on the
 * core's runtime and reaches the store's connection thread three times (people, interaction
 * history, coverage), so calling it from a `remember`, as this did, puts a blocking store read on
 * the main thread for every character typed, and a sync holding that connection stalls the
 * composer. It runs in a `LaunchedEffect` on the IO dispatcher instead, behind a short debounce:
 * the effect is cancelled and restarted on each new token, so a burst of keystrokes costs one
 * query, and a result whose token has already been superseded can never land. The list is capped
 * in the core, not here.
 *
 * Stale matches stay on screen for the debounce window rather than blanking, which is what makes
 * the dropdown feel steady instead of flickering on every character.
 *
 * It is a [Popup] rather than a child of the field's column, so it takes **no layout space**: Cc,
 * Bcc, Subject and the message stay where they were while the user types the first recipient.
 * Inline, the list appeared and vanished on every keystroke and took its height with it, so
 * everything below jumped down and back, and here that reaches further than it looks, because the
 * composer's header is measured and its height becomes the editor's top inset, so the message body
 * moved with it.
 *
 * The popup is **not focusable**: taking focus would close the software keyboard and stop the
 * keystrokes the list is completing, which is also why tapping a suggestion does not clear
 * [focused] before the tap lands. Its surface is opaque and shadowed because it now covers live
 * content.
 *
 * A popup is its own window, which the keyboard does not resize, so nothing keeps it off the
 * keyboard but [suggestionPlacement]: the list takes the room between the field and the keyboard,
 * or above the field where that is too little, and scrolls past it.
 */
@Composable
private fun RecipientSuggestionList(
    field: String,
    suggestionsFor: ((String) -> List<RecipientMatch>)?,
    onAccept: (String) -> Unit,
    fieldBounds: IntRect,
    window: IntRect,
    focused: Boolean,
) {
    if (suggestionsFor == null) {
        return
    }
    val token = currentRecipientToken(field)
    // The lambda is captured through `rememberUpdatedState` and the effect keys on the TOKEN
    // alone. Keying on the lambda instead would restart the query on every recomposition: the
    // composer builds it inline over the core handle, which Compose cannot prove stable, so it
    // gets a fresh identity each pass, and an effect that keeps restarting never finishes one.
    val lookup by rememberUpdatedState(suggestionsFor)
    var matches by remember { mutableStateOf(emptyList<RecipientMatch>()) }
    LaunchedEffect(token) {
        if (token.isEmpty()) {
            matches = emptyList()
            return@LaunchedEffect
        }
        delay(SUGGESTION_DEBOUNCE_MS)
        matches = withContext(Dispatchers.IO) { lookup(token) }
    }
    if (!focused || !shouldShowSuggestions(field, matches.map { it.email })) {
        return
    }
    val density = LocalDensity.current
    // The keyboard's inset when it is up, the navigation bar's when it is not: the window is
    // edge to edge, so both are drawn over it rather than taken out of it.
    val bottomInset = maxOf(
        WindowInsets.ime.getBottom(density),
        WindowInsets.navigationBars.getBottom(density),
    )
    val placement = suggestionPlacement(
        field = fieldBounds,
        visibleTop = window.top + WindowInsets.statusBars.getTop(density),
        visibleBottom = window.bottom - bottomInset,
        gapPx = with(density) { SUGGESTION_GAP.roundToPx() },
        minRoomPx = with(density) { SUGGESTION_MIN_ROOM.roundToPx() },
    )
    val gapPx = with(density) { SUGGESTION_GAP.roundToPx() }
    Popup(
        popupPositionProvider = remember(gapPx, placement.above) { BesideTheInput(gapPx, placement.above) },
        properties = PopupProperties(focusable = false),
    ) {
        Surface(
            modifier = Modifier
                .width(with(density) { window.width.toDp() })
                .heightIn(max = with(density) { placement.maxHeightPx.toDp() })
                .testTag("recipient-suggestions"),
            color = MaterialTheme.colorScheme.surface,
            shadowElevation = 6.dp,
            tonalElevation = 3.dp,
        ) {
            Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                matches.forEach { match ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onAccept(acceptRecipientSuggestion(field, match.email)) }
                            .padding(horizontal = 16.dp, vertical = 10.dp),
                    ) {
                        if (match.displayName.isNotEmpty()) {
                            Text(
                                text = match.displayName,
                                style = MaterialTheme.typography.bodyMedium,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Text(
                            text = match.email,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
    }
}
