// A recipient field's suggestion list: where it opens, beside the input, and the list itself.
// Split out of RecipientField.kt, which draws the field it serves.
package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
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
internal fun RecipientSuggestionList(
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
    val gapPx = with(density) { SUGGESTION_GAP.roundToPx() }
    val placement = suggestionPlacement(
        field = fieldBounds,
        visibleTop = window.top + WindowInsets.statusBars.getTop(density),
        visibleBottom = window.bottom - bottomInset,
        gapPx = gapPx,
        minRoomPx = with(density) { SUGGESTION_MIN_ROOM.roundToPx() },
    )
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
