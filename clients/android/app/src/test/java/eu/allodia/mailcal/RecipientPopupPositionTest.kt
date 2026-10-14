// Where a recipient field's floating suggestion list is put, and how tall it may be.
//
// Its own test because Compose's test framework cannot answer it: a `Popup` owns its own root, so
// the list's bounds read as (0, 0) however the provider placed it, and the screen-level assertion
// next door ("the form below does not move") passes whether the list hangs under the input, sits
// on top of it, or runs under the keyboard. All three happened, and only a device showed them. The
// decisions are what can be held; the rendering is verified by hand (docs/contacts.md §4).
package eu.allodia.mailcal

import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RecipientPopupPositionTest {
    // A field 320px wide sitting 200px down the form, 80px tall.
    private val field = IntRect(left = 24, top = 200, right = 344, bottom = 280)
    private val window = IntSize(1080, 2400)
    private val list = IntSize(1080, 180)

    // The window's visible part with the keyboard up: under the status bar, above the keyboard.
    private val visibleTop = 80
    private val keyboardTop = 1400

    private fun placement(at: IntRect) =
        suggestionPlacement(at, visibleTop, keyboardTop, gapPx = 12, minRoomPx = 400)

    @Test
    fun the_list_starts_below_the_input_at_the_window_edge() {
        val at = BesideTheInput(gapPx = 12, above = false)
            .calculatePosition(field, window, LayoutDirection.Ltr, list)

        // Not at the field's own left: the list spans the window, as the platform's pickers do,
        // because a phone's field is narrower than most addresses.
        assertEquals(IntOffset(0, 292), at)
    }

    @Test
    fun the_size_of_a_list_below_does_not_move_it() {
        // The anchor is the input, so a list of two rows and a list of ten start at the same
        // place: the trap in `Alignment.BottomStart`, which subtracts the popup's own height and
        // pulls it back up over the field.
        val short = BesideTheInput(gapPx = 12, above = false)
            .calculatePosition(field, window, LayoutDirection.Ltr, IntSize(1080, 90))
        val long = BesideTheInput(gapPx = 12, above = false)
            .calculatePosition(field, window, LayoutDirection.Ltr, IntSize(1080, 900))

        assertEquals(short, long)
    }

    @Test
    fun a_list_above_ends_just_over_the_field() {
        val low = field.translate(IntOffset(0, 1000))

        val at = BesideTheInput(gapPx = 12, above = true)
            .calculatePosition(low, window, LayoutDirection.Ltr, list)

        assertEquals(IntOffset(0, 1200 - 12 - 180), at)
    }

    @Test
    fun a_field_near_the_top_opens_below_and_stops_at_the_keyboard() {
        val chosen = placement(field)

        assertFalse(chosen.above)
        assertEquals("the room between the field and the keyboard", 1400 - 280 - 12, chosen.maxHeightPx)
    }

    @Test
    fun a_field_resting_on_the_keyboard_opens_above_it() {
        // The event editor's invitee field: the form scrolls it to just above the keyboard.
        val low = IntRect(left = 24, top = 1300, right = 344, bottom = 1380)

        val chosen = placement(low)

        assertTrue(chosen.above)
        assertEquals("the room between the status bar and the field", 1300 - 80 - 12, chosen.maxHeightPx)
    }

    @Test
    fun enough_room_below_wins_even_where_there_is_more_above() {
        // Under the field is where the eye already is; it moves only when that room is too small.
        val middle = IntRect(left = 24, top = 900, right = 344, bottom = 980)

        val chosen = placement(middle)

        assertFalse(chosen.above)
        assertEquals(1400 - 980 - 12, chosen.maxHeightPx)
    }

    @Test
    fun a_field_under_the_keyboard_never_asks_for_a_negative_height() {
        val hidden = IntRect(left = 24, top = 2000, right = 344, bottom = 2080)

        val chosen = placement(hidden)

        assertTrue(chosen.above)
        assertTrue(chosen.maxHeightPx >= 0)
    }
}
