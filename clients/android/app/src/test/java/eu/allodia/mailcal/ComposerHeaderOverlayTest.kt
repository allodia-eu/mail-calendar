// The header over the composer's editor keeps its own height when the keyboard leaves less room
// than it needs, as it does once a drafted reply's card sits under the address fields: its height
// is the editor's top inset, and its last rows are the card's checklist.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ComposerHeaderOverlayTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun the_header_keeps_its_height_when_the_keyboard_leaves_less_room() {
        var reported = 0
        compose.setContent {
            // What is left above the keyboard, well short of the header.
            Box(modifier = Modifier.height(200.dp)) {
                ComposerHeaderOverlay(scrollY = { 0 }, onHeight = { reported = it }) {
                    Spacer(modifier = Modifier.height(400.dp))
                    Text("Put the meeting in the calendar")
                }
            }
        }
        compose.waitForIdle()
        val row = compose.onNodeWithText("Put the meeting in the calendar").getUnclippedBoundsInRoot()
        assertTrue("the last row is laid out in full, not squeezed to ${row.height}", row.height > 10.dp)
        // The editor is told the whole height, so its text starts below the last row.
        val needed = with(compose.density) { 400.dp.roundToPx() }
        assertTrue("the editor's inset is the header's height, not the room ($reported px)", reported > needed)
    }
}
