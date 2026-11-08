// The rules behind a block's label (docs/calendar.md §4): how many lines a title may wrap over, when
// the corner glyphs earn their place, and what counts as breaking a word. Pure, so each is a check
// that can fail without drawing anything.
package eu.allodia.mailcal

import androidx.compose.ui.unit.dp
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.EventSwatch

class CalendarBlockLabelsTest {

    @Test
    fun a_title_wraps_over_every_whole_line_less_the_time_line() {
        // An hour at 56 dp leaves 52 dp: three whole 14 dp lines, one of them the time's.
        assertEquals(2, blockTitleLines(minutes = 60, hourHeight = 56.dp))
        // Three hours leave 164 dp: eleven lines, ten of them the title's.
        assertEquals(10, blockTitleLines(minutes = 180, hourHeight = 56.dp))
        // With no time line the title has all three.
        assertEquals(3, blockTitleLines(minutes = 60, hourHeight = 56.dp, withTime = false))
    }

    @Test
    fun a_larger_text_setting_earns_fewer_lines_rather_than_cut_ones() {
        // The same hour's 52 dp at a 1.3 font scale, where a 14 sp line is 18.2 dp: two whole lines,
        // one of them the time's.
        assertEquals(1, blockTitleLines(minutes = 60, hourHeight = 56.dp, line = 18.2.dp))
    }

    @Test
    fun a_block_with_room_for_one_line_keeps_it_for_the_title() {
        // A quarter hour at 56 dp leaves 12 dp: one 10 dp line, and no room for a time beneath it.
        assertEquals(1, blockTitleLines(minutes = 15, hourHeight = 56.dp))
    }

    @Test
    fun a_block_with_room_for_no_line_draws_no_title() {
        // A quarter hour at 40 dp leaves 8 dp, less than one line: no title, rather than half of one.
        assertEquals(0, blockTitleLines(minutes = 15, hourHeight = 40.dp))
    }

    @Test
    fun the_corner_glyphs_leave_the_title_at_least_36_dp() {
        val glyphs = cornerGlyphsWidth(count = 2, glyph = 11f, gap = 3f)
        assertEquals(25f, glyphs)
        assertTrue(cornerGlyphsFit(width = 64f, glyphsWidth = glyphs, gap = 3f, minTitle = 36f))
        assertFalse(cornerGlyphsFit(width = 63f, glyphsWidth = glyphs, gap = 3f, minTitle = 36f))
    }

    @Test
    fun an_event_with_no_glyph_has_nothing_to_fit() {
        assertEquals(0f, cornerGlyphsWidth(count = 0, glyph = 11f, gap = 3f))
        assertFalse(cornerGlyphsFit(width = 500f, glyphsWidth = 0f, gap = 3f, minTitle = 36f))
    }

    @Test
    fun a_break_between_two_letters_is_a_broken_word() {
        val title = "Quarterly planning"
        assertFalse(breaksAWord(title, listOf(10, title.length)))
        assertTrue(breaksAWord(title, listOf(5, title.length)))
    }

    @Test
    fun a_break_after_a_hyphen_is_a_wrap() {
        assertFalse(breaksAWord("Check-in call", listOf(6, 13)))
    }

    @Test
    fun the_core_is_asked_once_per_colour_and_theme() {
        var asked = 0
        EventSwatches.source = { hex, dark ->
            asked++
            EventSwatch(fill = "#eeeeee", accent = hex, text = if (dark) "#ffffff" else "#000000")
        }
        val light = EventSwatches.colors("#2f6fa8", dark = false)
        EventSwatches.colors("#2f6fa8", dark = false)
        val dark = EventSwatches.colors("#2f6fa8", dark = true)
        assertEquals(2, asked)
        assertEquals(parseHexColor("#2f6fa8"), light.accent)
        assertEquals(parseHexColor("#000000"), light.text)
        assertEquals(parseHexColor("#ffffff"), dark.text)
    }

    @After
    fun restoreTheSuitesColours() {
        EventSwatches.source = ::testEventSwatch
    }
}
