// The reading page scrolls as one (ReadingPage.kt, docs/reading-actions.md): the header goes up
// first as the body scrolls toward its end, and comes back once the body is at its top; a header
// taller than the screen scrolls by itself.
//
// The HTML body's share of this is the WebView's (ReadingWebViewTest.kt); the plain-text body is
// Compose's own scroll and goes through the same connection, so it stands in for both here.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingPageTest {
    @get:Rule val rule = createComposeRule()

    private fun header(height: Float = 300f) = CollapsingHeader(mutableFloatStateOf(0f)).apply { this.height = height }

    @Test
    fun aScrollTowardTheEndTakesTheHeaderAwayUpToItsHeight() {
        val header = header()
        assertEquals(-120f, header.scroll(-120f))
        assertEquals(120f, header.offset)
        // Only what is left of the header is taken; the body scrolls the rest.
        assertEquals(-180f, header.scroll(-400f))
        assertEquals(300f, header.offset)
    }

    @Test
    fun aScrollBackReturnsTheHeaderAndNoMore() {
        val header = header()
        header.scroll(-200f)
        assertEquals(200f, header.scroll(500f))
        assertEquals(0f, header.offset)
    }

    @Test
    fun theBodyScrollsOnlyOnceTheHeaderHasGoneAndTheHeaderReturnsOnlyAtTheBodysTop() {
        val header = CollapsingHeader(mutableFloatStateOf(0f))
        val scroll = androidx.compose.foundation.ScrollState(0)
        rule.setContent {
            AppTheme {
                ReadingPage(
                    state = header,
                    modifier = Modifier.fillMaxSize(),
                    header = { Box(Modifier.fillMaxWidth().height(200.dp).testTag("header")) },
                    body = {
                        Column(Modifier.fillMaxSize().verticalScroll(scroll).testTag("body")) {
                            repeat(60) { Text("Line $it", modifier = Modifier.height(40.dp)) }
                        }
                    },
                )
            }
        }
        rule.onNodeWithTag("body").performTouchInput { swipeUp(startY = top + 150f, endY = top + 50f) }
        rule.waitForIdle()
        // A short scroll is all header: the body has not moved.
        assertTrue(header.offset > 0f)
        assertEquals(0, scroll.value)

        rule.onNodeWithTag("body").performTouchInput { swipeUp() }
        rule.onNodeWithTag("body").performTouchInput { swipeUp() }
        rule.waitForIdle()
        assertEquals(header.height, header.offset)
        assertTrue(scroll.value > 0)

        // Back down: the body returns to its top before the header comes back.
        val before = scroll.value
        rule.onNodeWithTag("body").performTouchInput { swipeDown(startY = top + 10f, endY = top + 60f) }
        rule.waitForIdle()
        assertTrue(scroll.value < before)
        assertEquals(header.height, header.offset)
    }

    @Test
    fun aHeaderTallerThanThePageScrollsByItself() {
        val header = CollapsingHeader(mutableFloatStateOf(0f))
        rule.setContent {
            AppTheme {
                Box(Modifier.size(width = 320.dp, height = 400.dp)) {
                    ReadingPage(
                        state = header,
                        modifier = Modifier.fillMaxSize(),
                        // Twenty attachments, or a long conversation: more than the page holds.
                        header = { Box(Modifier.fillMaxWidth().height(900.dp).testTag("header")) },
                        body = { Box(Modifier.fillMaxSize()) },
                    )
                }
            }
        }
        rule.onNodeWithTag("header").performTouchInput { swipeUp() }
        rule.waitForIdle()
        assertTrue(header.offset > 0f)
    }
}
