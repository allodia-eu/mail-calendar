// Scrolling the composer from its address header (ComposerHeaderScroll.kt). The WebView owns the
// one scroll and the header is an overlay above it, so without the forwarding a drag that starts on
// From, To or Subject moves nothing, while the same drag on the message moves both.
package eu.allodia.mailcal

import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.shadows.ShadowDialog

@RunWith(RobolectricTestRunner::class)
class ComposerHeaderScrollTest {
    @get:Rule val compose = createComposeRule()

    /** A page that scrolls freely between 0 and [max], as the WebView clamps it. */
    private class Page(private val max: Int) {
        var y = 0
        val asked = mutableListOf<Int>()

        fun scrollBy(dy: Int): Int {
            asked += dy
            val before = y
            y = (y + dy).coerceIn(0, max)
            return y - before
        }
    }

    @Test
    fun whole_pixels_reach_the_page_and_the_fraction_waits_for_the_next_drag() {
        val page = Page(max = 1000)
        val forwarder = HeaderScrollForwarder(page::scrollBy)

        assertEquals(0.6f, forwarder.dispatch(0.6f))
        assertEquals(0.6f, forwarder.dispatch(0.6f))

        // Neither drag alone was a pixel; together they were one, and the rest is kept.
        assertEquals(listOf(1), page.asked)
        assertEquals(1, page.y)
    }

    @Test
    fun a_drag_back_towards_the_top_scrolls_the_page_back() {
        val page = Page(max = 1000)
        val forwarder = HeaderScrollForwarder(page::scrollBy)

        forwarder.dispatch(40f)
        forwarder.dispatch(-15f)

        assertEquals(25, page.y)
    }

    /** Answering what the page took, rather than what was asked, is what ends a fling at the edge. */
    @Test
    fun at_the_end_of_the_page_only_what_moved_is_answered() {
        val page = Page(max = 30)
        val forwarder = HeaderScrollForwarder(page::scrollBy)

        assertEquals(30f, forwarder.dispatch(50f))
        assertEquals(0f, forwarder.dispatch(10f))
        assertEquals(30, page.y)
    }

    @Test
    fun at_the_top_a_drag_down_is_not_taken() {
        val page = Page(max = 1000)
        val forwarder = HeaderScrollForwarder(page::scrollBy)

        assertEquals(0f, forwarder.dispatch(-20f))
        assertEquals(0, page.y)
    }

    /** The real composer: a swipe up that starts on Subject scrolls the message. */
    @Test
    fun a_swipe_on_the_header_scrolls_the_editor() {
        compose.setContent {
            AppTheme {
                RichComposeMessageDialog(
                    request = ComposeContext(testOpening(), initialFrom = null),
                    onDismiss = {},
                    onSubmitRich = { _ -> true },
                    accounts = emptyList(),
                )
            }
        }
        compose.waitForIdle()
        val editor = checkNotNull(webViewIn(checkNotNull(ShadowDialog.getLatestDialog().window).decorView))
        assertEquals(0, editor.scrollY)

        compose.onNodeWithText(L10n.compose_subject(RuntimeEnvironment.getApplication()))
            .performTouchInput { swipeUp() }
        compose.waitForIdle()

        assertTrue("the editor must scroll, at ${editor.scrollY}", editor.scrollY > 0)
    }

    private fun webViewIn(view: View): WebView? = when (view) {
        is WebView -> view
        is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { webViewIn(view.getChildAt(it)) }
        else -> null
    }
}
