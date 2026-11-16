// Scrolling the composer from its address header. The WebView owns the composer's one scroll and
// the header is a native overlay that follows it (RichComposeScreen.kt), so a drag that starts on
// From, To or Subject lands on nothing that scrolls. The frame here takes those drags and flings and
// hands them to the WebView, so the header and the message move as one page.
package eu.allodia.mailcal

import android.content.Context
import android.webkit.WebView
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.rememberScrollableState
import androidx.compose.foundation.gestures.scrollable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.Constraints

/**
 * Feeds drag deltas, in fractional pixels, to a target that moves in whole ones, and answers how
 * much of each it took. [scrollBy] moves the target and returns how far it actually went, which is
 * less than asked at either end of the page; answering that is what stops a fling there.
 */
internal class HeaderScrollForwarder(private val scrollBy: (Int) -> Int) {
    private var remainder = 0f

    fun dispatch(delta: Float): Float {
        val wanted = delta + remainder
        val whole = wanted.toInt()
        if (whole == 0) {
            remainder = wanted
            return delta
        }
        val moved = scrollBy(whole)
        if (moved != whole) {
            remainder = 0f
            return moved.toFloat()
        }
        remainder = wanted - whole
        return delta
    }
}

/**
 * The composer's editor, which knows how far its page scrolls.
 *
 * A `scrollBy` or `scrollTo` on a WebView sets the view's offset without holding it to the page:
 * the page stops at its end, while the view's `scrollY`, which the header overlay follows, carries
 * on past it. The header then leaves the message behind and a blank band opens between them. So
 * every scroll this app starts goes through [scrollPageBy], which keeps the offset on the page.
 */
internal open class ComposerWebView(context: Context) : WebView(context) {
    /** The furthest the page scrolls, in pixels: its height less the part on screen. */
    open val maxScrollY: Int
        get() = (computeVerticalScrollRange() - computeVerticalScrollExtent()).coerceAtLeast(0)

    /**
     * Moves the page by [dy] pixels and answers how far it went. An offset already past the end, as
     * a page that has just grown shorter can leave, is brought back first, so the answer always has
     * the sign of [dy] and the header lands back on the message.
     */
    fun scrollPageBy(dy: Int): Int {
        val max = maxScrollY
        val before = scrollY.coerceIn(0, max)
        val after = (before + dy).coerceIn(0, max)
        if (after != scrollY) scrollTo(scrollX, after)
        return after - before
    }
}

/**
 * The header's frame: [content] drawn [scrollY] pixels up, clipped to the part still on screen, and
 * every vertical drag or fling on it sent to [webView].
 *
 * The frame itself never moves; its height shrinks as the header scrolls away. That is what makes
 * the drag work at all: a gesture read on a node that moves with the scroll sees the finger stand
 * still, because the node follows it. And because the frame is only as tall as what is visible, a
 * touch below the header still reaches the message.
 */
@Composable
internal fun ComposerHeaderFrame(
    scrollY: () -> Int,
    webView: () -> ComposerWebView?,
    content: @Composable () -> Unit,
) {
    val target by rememberUpdatedState(webView)
    val forwarder = remember { HeaderScrollForwarder { dy -> target()?.scrollPageBy(dy) ?: 0 } }
    val state = rememberScrollableState(forwarder::dispatch)
    Box(
        modifier = Modifier
            .fillMaxWidth()
            // Reversed, as a vertical scroll is: a finger moving up scrolls the page down.
            .scrollable(state, Orientation.Vertical, reverseDirection = true)
            .clipToBounds()
            .layout { measurable, constraints ->
                val placeable = measurable.measure(
                    constraints.copy(minHeight = 0, maxHeight = Constraints.Infinity),
                )
                val offset = scrollY().coerceIn(0, placeable.height)
                // Never taller than the room it is given. A header with many files outgrows the
                // space above the keyboard, and a layout that reports more than its constraints
                // allow is centred in them, which would draw it half its overflow above the page.
                val visible = (placeable.height - offset).coerceAtMost(constraints.maxHeight)
                layout(placeable.width, visible) {
                    placeable.place(0, -offset)
                }
            },
    ) {
        content()
    }
}
