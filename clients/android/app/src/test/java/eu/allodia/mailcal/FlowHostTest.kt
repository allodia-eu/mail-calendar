// The flow host's foot (docs/ai.md, "The flow host"): Back, the page indicator and the primary on
// one row while they fit, never drawn over one another.
package eu.allodia.mailcal

import androidx.compose.material3.Text
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
class FlowHostTest {
    @get:Rule val compose = createComposeRule()

    // The indicator is centred on the foot, so a primary wider than half of what the indicator
    // leaves reaches under it even while the three together fit the row.
    @Test
    @Config(qualifiers = "w420dp")
    @GraphicsMode(GraphicsMode.Mode.NATIVE)
    fun the_indicator_is_drawn_clear_of_a_wide_primary() {
        compose.setContent {
            AppTheme {
                FlowHost(
                    FlowChrome(
                        title = "Flow",
                        close = FlowAction("Close", FREE, {}),
                        back = FlowAction("Back", FREE, {}),
                        primary = FlowAction("Save and close now", FREE, {}),
                        pages = FlowPages(count = 4, current = 1),
                    ),
                ) { Text("page") }
            }
        }
        compose.waitForIdle()
        val indicator = compose.onNodeWithTag(FLOW_INDICATOR_TAG).getUnclippedBoundsInRoot()
        for (tag in listOf(FLOW_BACK_TAG, FLOW_PRIMARY_TAG)) {
            val button = compose.onNodeWithTag(tag).getUnclippedBoundsInRoot()
            val apart = indicator.right <= button.left || button.right <= indicator.left ||
                indicator.bottom <= button.top || button.bottom <= indicator.top
            assertTrue("$tag $button overlaps the indicator $indicator", apart)
        }
    }
}
