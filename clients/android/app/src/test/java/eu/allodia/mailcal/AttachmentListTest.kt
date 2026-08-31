// The reading screen's attachment list (ReadingScreen.kt).
//
// The list sits ABOVE the message in a column that does not scroll, so its height is not cosmetic:
// left to grow, twenty files push the body clean off the bottom of the screen and neither the mail
// nor the last few attachments can be reached. One attachment cannot show that, which is why the
// harness fixture that proves it end to end (10-many-attachments.eml) had to be added too.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AttachmentRow

private fun files(count: Int) = (1..count).map {
    AttachmentRow(
        id = it.toUInt(),
        fileName = "quote-%02d.csv".format(it),
        mediaType = "text/csv",
        size = 26uL,
    )
}

@RunWith(RobolectricTestRunner::class)
class AttachmentListTest {
    @get:Rule val rule = createComposeRule()

    // `DpRect` exposes its edges, not its extent.
    private fun rowsHeight(): Dp =
        rule.onNodeWithTag(ATTACHMENT_ROWS_TAG).getBoundsInRoot().let { it.bottom - it.top }

    private fun show(count: Int) {
        rule.setContent {
            AttachmentList(attachments = files(count), onSave = {}, onOpen = {})
        }
    }

    @Test
    fun aLongListStopsAtTheCapInsteadOfPushingTheMessageOffScreen() {
        show(20)
        val height = rowsHeight()
        assertTrue(
            "twenty attachments took $height, past the ${ATTACHMENT_LIST_CAP} cap",
            height <= ATTACHMENT_LIST_CAP,
        )
    }

    @Test
    fun everyFileIsStillReachable() {
        // The half a cap alone does not buy: the rows scroll, so the twentieth file is as
        // reachable as the first. Without this a "fix" that merely clipped the list would pass.
        show(20)
        rule.onNodeWithText("quote-20.csv").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun aShortListHugsItsRowsRatherThanReservingTheCap() {
        // Two attachments must not leave a blank strip over the message, the failure mode of
        // capping with a fixed height instead of `heightIn`.
        show(2)
        val height = rowsHeight()
        assertTrue("two attachments reserved $height", height < ATTACHMENT_LIST_CAP)
        assertTrue("two attachments took no room at all", height > 0.dp)
    }
}
