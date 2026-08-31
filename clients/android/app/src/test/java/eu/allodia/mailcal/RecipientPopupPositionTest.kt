// Where a recipient field's floating suggestion list is put.
//
// Its own test because Compose's test framework cannot answer it: a `Popup` owns its own root, so
// the list's bounds read as (0, 0) however the provider placed it, and the screen-level assertion
// next door ("the form below does not move") passes whether the list hangs under the input or sits
// squarely on top of it. It did sit on top of it, `Alignment.TopStart` aligns a popup inside its
// PARENT's bounds, which here is the field's whole column, so the list covered the very text it was
// completing and only a real emulator showed it. The decision is what can be held; the rendering is
// verified by hand (docs/contacts.md §4).
package eu.allodia.mailcal

import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import org.junit.Assert.assertEquals
import org.junit.Test

class RecipientPopupPositionTest {
    // A field 320px wide sitting 200px down the form, 80px tall.
    private val field = IntRect(left = 24, top = 200, right = 344, bottom = 280)
    private val window = IntSize(1080, 2400)
    private val list = IntSize(320, 180)

    @Test
    fun the_list_starts_below_the_input_not_over_it() {
        val at = UnderTheInput(gapPx = 12)
            .calculatePosition(field, window, LayoutDirection.Ltr, list)

        assertEquals(IntOffset(24, 292), at)
    }

    @Test
    fun the_list_is_left_aligned_with_the_input_it_completes() {
        // Not with the screen: an indented field would otherwise offer a list that starts
        // somewhere else entirely.
        val indented = field.translate(IntOffset(120, 0))

        val at = UnderTheInput(gapPx = 0)
            .calculatePosition(indented, window, LayoutDirection.Ltr, list)

        assertEquals(144, at.x)
    }

    @Test
    fun the_size_of_the_list_does_not_move_it() {
        // The anchor is the input, so a list of two rows and a list of ten start at the same place
        // the trap in `Alignment.BottomStart`, which subtracts the popup's own height and pulls
        // it back up over the field.
        val short = UnderTheInput(gapPx = 12)
            .calculatePosition(field, window, LayoutDirection.Ltr, IntSize(320, 90))
        val long = UnderTheInput(gapPx = 12)
            .calculatePosition(field, window, LayoutDirection.Ltr, IntSize(320, 900))

        assertEquals(short, long)
    }
}
