// The composer header's Cc/Bcc reveal and its quote-style picker. By default only From/To/Subject
// show (as Gmail and Thunderbird do); Cc/Bcc hide behind the chevron on the To row. That keeps the
// pinned header short enough to leave the editor room, so "are they hidden, and does the chevron
// reveal them" is the contract this screen has to keep.
//
// The quote-style picker is the other half: it appears only when the user opted into per-message
// styling, and it has to lay out inside the row at phone width.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.QuoteStyleKind

private val ALICE = AccountRow("acct-1", "alice@test.local", expanded = true)

// A narrow phone, where the row is tightest. The tablet lays the same row out fine, so the width is the
// whole point of the test. (Robolectric's own screen is narrower still, and wins, which only makes
// the row tighter, so the assertions hold either way; they measure the composed row, not this.)
private val PHONE_WIDTH = 360.dp

// Adjacent segmented buttons share one divider stroke, so their bounds meet with ~1dp of overlap by
// design. Rounding in the dp conversion adds a fraction more; this is the slack the layout
// assertions allow, well below the tens of dp the overflow bug produced.
private val SEAM = 2.dp

@RunWith(RobolectricTestRunner::class)
class ComposerHeaderFieldsTest {
    @get:Rule val compose = createComposeRule()

    private fun header(
        startExpanded: Boolean = false,
        style: QuoteStyleKind? = null,
        width: Dp = PHONE_WIDTH,
    ) {
        var showCcBcc by mutableStateOf(startExpanded)
        compose.setContent {
            Box(modifier = Modifier.width(width)) {
                ComposerHeaderFields(
                    accounts = listOf(ALICE),
                    from = ALICE,
                    onFrom = {},
                    to = "",
                    onTo = {},
                    cc = "",
                    onCc = {},
                    bcc = "",
                    onBcc = {},
                    subject = "",
                    onSubject = {},
                    showsSubject = true,
                    showCcBcc = showCcBcc,
                    onToggleCcBcc = { showCcBcc = !showCcBcc },
                    style = style,
                    onStyle = {},
                )
            }
        }
    }

    @Test
    fun cc_and_bcc_are_hidden_by_default() {
        header(startExpanded = false)

        // The default set the reference apps show.
        compose.onNodeWithText("To").assertIsDisplayed()
        compose.onNodeWithText("Subject").assertIsDisplayed()
        compose.onNodeWithText("Cc").assertDoesNotExist()
        compose.onNodeWithText("Bcc").assertDoesNotExist()
    }

    @Test
    fun the_chevron_reveals_cc_and_bcc() {
        header(startExpanded = false)

        compose.onNodeWithContentDescription("Show Cc and Bcc").performClick()
        compose.waitForIdle()

        compose.onNodeWithText("Cc").assertIsDisplayed()
        compose.onNodeWithText("Bcc").assertIsDisplayed()
    }

    @Test
    fun there_is_no_quote_style_picker_unless_the_caller_passes_a_style() {
        // The default: the user hasn't opted into per-message styling, so the reply just uses the
        // app default and the composer shows no picker at all.
        header(style = null)

        compose.onNodeWithText("Indented").assertDoesNotExist()
        compose.onNodeWithText("Line + header").assertDoesNotExist()
    }

    @Test
    fun the_quote_style_segments_split_the_row_evenly_at_phone_width() {
        header(style = QuoteStyleKind.INDENTED)

        val root = compose.onRoot().getUnclippedBoundsInRoot()
        val rowRight = root.right
        val indented = compose.onNodeWithText("Indented").assertIsDisplayed()
            .getUnclippedBoundsInRoot()
        val lineHeader = compose.onNodeWithText("Line + header").assertIsDisplayed()
            .getUnclippedBoundsInRoot()

        // Sized to its own content, each SegmentedButton pushes the second
        // segment past the row, painting over the first segment's right edge and spilling outside
        // the dialog's rounded corner. Giving each `weight(1f)` makes them share the width instead,
        // which is what these three assertions pin: equal halves, starting at the row's left edge
        // and ending at its right.
        //
        // Adjacent segments share their divider stroke, so a 1dp overlap at the seam is Material's
        // design, not the bug, the bug was a segment painting *over* its neighbour's content.
        val indentedWidth = indented.right - indented.left
        val lineHeaderWidth = lineHeader.right - lineHeader.left
        assertTrue(
            "segments must be equally wide: $indentedWidth vs $lineHeaderWidth",
            abs(indentedWidth.value - lineHeaderWidth.value) <= SEAM.value,
        )
        assertTrue(
            "segments must sit side by side, not on top of each other: $indented vs $lineHeader",
            indented.right <= lineHeader.left + SEAM,
        )
        assertTrue(
            "the picker must fill the row and not spill past it: ${lineHeader.right} vs $rowRight",
            abs(lineHeader.right.value - rowRight.value) <= SEAM.value,
        )
    }
}
