// The reading page is one scroll (ReadingPage.kt, docs/reading-actions.md): the header and the
// message move together, and a fling runs from the message to the subject without stopping at the
// message's top. The message here is Compose text; the WebView's share is that it never scrolls by
// itself (ReadingWebViewTest.kt), so the page is the only thing that moves either way.
package eu.allodia.mailcal

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingPageTest {
    @get:Rule val rule = createComposeRule()

    private val scroll = ScrollState(0)

    private fun show(lines: Int = 80) {
        rule.setContent {
            AppTheme {
                ReadingPage(
                    scroll = scroll,
                    page = Color.White,
                    modifier = Modifier.fillMaxSize().testTag("page"),
                    header = { Box(Modifier.fillMaxWidth().height(300.dp).testTag("header")) },
                    body = {
                        Column {
                            repeat(lines) { Text("Line $it", modifier = Modifier.height(40.dp)) }
                        }
                    },
                )
            }
        }
    }

    private fun headerTop() = rule.onNodeWithTag("header").fetchSemanticsNode().positionInRoot.y

    @Test
    fun aDragOnTheMessageMovesTheHeaderWithIt() {
        show()
        rule.onNodeWithTag("page").performTouchInput { swipeUp(startY = bottom - 20f, endY = bottom - 220f) }
        rule.waitForIdle()
        assertTrue(scroll.value > 0)
        // One page: the header has gone up by exactly what the page did.
        assertEquals(-scroll.value.toFloat(), headerTop(), 1f)
    }

    @Test
    fun aFlingBackFromDeepInTheMessageRunsOnToTheSubject() {
        show()
        rule.runOnIdle { runBlocking { scroll.scrollTo(1500) } }
        rule.onNodeWithTag("page").performTouchInput {
            swipeDown(startY = top + 20f, endY = bottom - 20f, durationMillis = 80)
        }
        rule.waitForIdle()
        assertEquals(0, scroll.value)
        assertEquals(0f, headerTop(), 1f)
    }

    @Test
    fun aShortMessageStillLetsTheHeaderScrollAway() {
        // The page is at least a screen below the header, so the header can always go.
        show(lines = 2)
        rule.onNodeWithTag("page").performTouchInput { swipeUp() }
        rule.waitForIdle()
        assertTrue(scroll.value > 0)
    }
}
