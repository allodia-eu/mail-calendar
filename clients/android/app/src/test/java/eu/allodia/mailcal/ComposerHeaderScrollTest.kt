// Scrolling the composer from its address header (ComposerHeaderScroll.kt). The WebView owns the
// one scroll and the header is an overlay above it, so without the forwarding a drag that starts on
// From, To or Subject moves nothing, while the same drag on the message moves both.
package eu.allodia.mailcal

import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertTopPositionInRootIsEqualTo
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.unit.dp
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

    /** A page that scrolls between 0 and [max], as [ComposerWebView.scrollPageBy] holds it. */
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

    /** The editor with a page [max] pixels taller than the part on screen. */
    private fun editor(max: Int) = object : ComposerWebView(RuntimeEnvironment.getApplication()) {
        override val maxScrollY = max
    }

    /**
     * A WebView's own `scrollBy` carries its offset past the end of the page, and the header follows
     * that offset: past the end it would leave the message behind, with a blank band under the files.
     */
    @Test
    fun the_offset_stops_at_the_end_of_the_page() {
        val editor = editor(max = 500)

        assertEquals(500, editor.scrollPageBy(800))
        assertEquals(500, editor.scrollY)
        assertEquals(0, editor.scrollPageBy(300))
        assertEquals(500, editor.scrollY)
        assertEquals(-500, editor.scrollPageBy(-900))
        assertEquals(0, editor.scrollY)
    }

    /** A page that grew shorter under the offset: the next drag starts from its end, not past it. */
    @Test
    fun an_offset_past_the_end_is_brought_back_to_the_page() {
        val editor = editor(max = 500)
        editor.scrollTo(0, 900)

        assertEquals(-10, editor.scrollPageBy(-10))
        assertEquals(490, editor.scrollY)
    }

    /** Scrolling on past the end moves nothing, and leaves the offset at the end of the page. */
    @Test
    fun scrolling_on_past_the_end_answers_nothing_moved() {
        val editor = editor(max = 500)
        editor.scrollTo(0, 900)

        assertEquals(0, editor.scrollPageBy(40))
        assertEquals(500, editor.scrollY)
    }

    /**
     * A header taller than the room it is given (twenty files above a keyboard) starts at the top of
     * the page and moves with it, rather than being centred, half its overflow above the message.
     */
    @Test
    fun a_header_taller_than_its_room_starts_at_the_top_and_follows_the_page() {
        var scrolled by mutableIntStateOf(0)
        compose.setContent {
            Box(Modifier.height(400.dp)) {
                ComposerHeaderFrame(scrollY = { scrolled }, webView = { null }) {
                    Box(Modifier.fillMaxWidth().height(1000.dp).testTag("header"))
                }
            }
        }
        compose.onNodeWithTag("header").assertTopPositionInRootIsEqualTo(0.dp)

        scrolled = with(compose.density) { 100.dp.roundToPx() }
        compose.onNodeWithTag("header").assertTopPositionInRootIsEqualTo((-100).dp)
    }

    /** The real composer: a swipe up that starts on Subject scrolls the message. */
    @Test
    fun a_swipe_on_the_header_scrolls_the_editor() {
        compose.setContent {
            CompositionLocalProvider(LocalComposerEditorFactory provides { _ -> editor(max = 2000) }) {
                AppTheme {
                    RichComposeMessageDialog(
                        request = ComposeContext(testOpening(), initialFrom = null),
                        onDismiss = {},
                        onSubmitRich = { _ -> true },
                        accounts = emptyList(),
                    )
                }
            }
        }
        compose.waitForIdle()
        val editor = checkNotNull(webViewIn(checkNotNull(ShadowDialog.getLatestDialog().window).decorView))
        assertEquals(0, editor.scrollY)

        compose.onNodeWithText(L10n.compose_subject(RuntimeEnvironment.getApplication()))
            .performTouchInput { swipeUp() }
        compose.waitForIdle()

        assertTrue("the editor must scroll, at ${editor.scrollY}", editor.scrollY in 1..2000)
    }

    private fun webViewIn(view: View): WebView? = when (view) {
        is WebView -> view
        is ViewGroup -> (0 until view.childCount).firstNotNullOfOrNull { webViewIn(view.getChildAt(it)) }
        else -> null
    }
}
