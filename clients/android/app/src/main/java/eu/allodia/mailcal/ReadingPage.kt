// The reading page: everything about the message that is not the message (subject, the rest of the
// conversation, who sent it, its banners, files and invitation) scrolls away with the body it
// belongs to, between the top app bar and the answers, which stay put (docs/reading-actions.md).
//
// The body brings its own scroll (a WebView, or the plain text's column), so the two are one
// gesture through nested scrolling: a scroll toward the end takes the header away first, and a
// scroll back brings it back once the body is at its top. The header also scrolls by itself, so
// one taller than the screen, a long conversation or twenty attachments, can still be passed.
package eu.allodia.mailcal

import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.ScrollableState
import androidx.compose.foundation.gestures.scrollable
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.MutableFloatState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.unit.Constraints
import kotlin.math.roundToInt

/**
 * How far the header has gone up, in pixels, between none of it and all of it.
 *
 * Deltas use Compose's sign: negative is a scroll toward the end, the finger moving up.
 */
internal class CollapsingHeader(private val offsetState: MutableFloatState = mutableFloatStateOf(0f)) {
    var offset: Float
        get() = offsetState.floatValue
        private set(value) {
            offsetState.floatValue = value
        }

    /** The header's height, measured with the page. */
    var height by mutableFloatStateOf(0f)

    /** Takes what it can of [delta] by moving the header; returns what it took. */
    fun scroll(delta: Float): Float {
        val next = (offset - delta).coerceIn(0f, height)
        val taken = offset - next
        offset = next
        return taken
    }

    /** The body's scroll: toward the end the header goes first, back it returns once the body is at its top. */
    val connection = object : NestedScrollConnection {
        override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset =
            if (available.y < 0) Offset(0f, scroll(available.y)) else Offset.Zero

        override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset =
            if (available.y > 0) Offset(0f, scroll(available.y)) else Offset.Zero
    }

    /** A drag or fling on the header itself. */
    val headerScroll = ScrollableState { delta -> scroll(delta) }
}

@Composable
internal fun ReadingPage(
    state: CollapsingHeader,
    header: @Composable () -> Unit,
    body: @Composable () -> Unit,
    modifier: Modifier = Modifier,
) {
    Layout(
        contents = listOf(
            { Box(modifier = Modifier.scrollable(state.headerScroll, Orientation.Vertical)) { header() } },
            body,
        ),
        modifier = modifier.clipToBounds().nestedScroll(state.connection),
    ) { (headerMeasurables, bodyMeasurables), constraints ->
        val header = headerMeasurables.single().measure(constraints.copy(minHeight = 0, maxHeight = Constraints.Infinity))
        // The body is the page's full height: it is what is left once the header has gone.
        val body = bodyMeasurables.single().measure(constraints.copy(minHeight = constraints.maxHeight))
        state.height = header.height.toFloat()
        val offset = state.offset.coerceIn(0f, header.height.toFloat()).roundToInt()
        layout(constraints.maxWidth, constraints.maxHeight) {
            header.place(0, -offset)
            body.place(0, header.height - offset)
        }
    }
}
