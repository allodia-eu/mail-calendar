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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
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

/** The gap between a recipient input and the list hanging under it. */
private val SUGGESTION_GAP = 4.dp

/**
 * Puts a field's suggestion list **under** the input, never over it.
 *
 * It takes an explicit provider to get there, and nothing about the call site says so. A [Popup]'s
 * `alignment` positions it inside its PARENT's bounds, here the recipient field's whole column:
 * so the obvious `Alignment.TopStart` lands the list on top of the input, covering the very text it
 * is completing. The popup adds no height of its own, so that column's bottom edge *is* the bottom
 * of the input, which is the anchor wanted.
 *
 * A named class rather than a lambda because Compose's test framework cannot see where a popup
 * actually lands, a popup owns its own root, so its bounds read as (0, 0) whatever the provider
 * did. The decision is therefore what gets asserted (`RecipientPopupPositionTest`).
 */
internal class UnderTheInput(private val gapPx: Int) : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset = IntOffset(anchorBounds.left, anchorBounds.bottom + gapPx)
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
    // The input's own width, so the floating list below matches the field rather than the screen.
    var inputWidthPx by remember { mutableIntStateOf(0) }
    // Only the focused field offers suggestions. Moving from To to Cc must close To's list:
    // harmless while it sat in the layout, and covering live content the moment it floats.
    var focused by remember { mutableStateOf(false) }

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

    Column(modifier = modifier.fillMaxWidth()) {
        if (committed.isNotEmpty()) {
            FlowRow(modifier = Modifier.fillMaxWidth().testTag("recipient-pills")) {
                committed.forEachIndexed { index, recipient ->
                    InputChip(
                        selected = false,
                        onClick = { onValue(removeRecipient(value, index)) },
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
                onValue(recipientFieldText(committed, typed.text))
            },
            modifier = Modifier
                .fillMaxWidth()
                .testTag("recipient-input-$label")
                .onSizeChanged { inputWidthPx = it.width }
                .onFocusChanged { focused = it.isFocused }
                .focusRequester(focusRequester),
            singleLine = true,
            label = { Text(label) },
            trailingIcon = trailing,
        )

        RecipientSuggestionList(
            field = value,
            suggestionsFor = suggestionsFor,
            onAccept = onValue,
            widthPx = inputWidthPx,
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
 */
@Composable
private fun RecipientSuggestionList(
    field: String,
    suggestionsFor: ((String) -> List<RecipientMatch>)?,
    onAccept: (String) -> Unit,
    widthPx: Int,
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
    val width = with(LocalDensity.current) { widthPx.toDp() }
    val gapPx = with(LocalDensity.current) { SUGGESTION_GAP.roundToPx() }
    Popup(
        popupPositionProvider = remember(gapPx) { UnderTheInput(gapPx) },
        properties = PopupProperties(focusable = false),
    ) {
        Surface(
            modifier = Modifier.width(width).testTag("recipient-suggestions"),
            shape = RoundedCornerShape(8.dp),
            color = MaterialTheme.colorScheme.surface,
            shadowElevation = 6.dp,
            tonalElevation = 3.dp,
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                matches.forEach { match ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onAccept(acceptRecipientSuggestion(field, match.email)) }
                            .padding(horizontal = 12.dp, vertical = 8.dp),
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
