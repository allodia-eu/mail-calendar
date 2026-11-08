// A block as the grid builds and lays it out (docs/calendar.md §1 and §4): the core's event colours,
// the hold's fainter tint, the spoken repeat, and a title that wraps by word, never through one.
//
// NATIVE graphics, because under Robolectric's default canvas text measures as nothing and every
// layout assertion would pass on an empty line.
package eu.allodia.mailcal

import androidx.compose.ui.graphics.painter.ColorPainter
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.GraphicsMode
import uniffi.mailcal_bindings.ResponseStatus

private val THEME = SurfaceTheme(
    primary = Color.Blue,
    outlineVariant = Color.Gray,
    error = Color.Red,
    surface = Color.White,
    blockBase = TextStyle.Default,
    weekday = TextStyle.Default,
    weekdayToday = TextStyle.Default,
    dayNumber = TextStyle.Default,
    dayNumberToday = TextStyle.Default,
    hour = TextStyle.Default,
    weekLabel = TextStyle.Default,
    weekNumber = TextStyle.Default,
    allDay = TextStyle.Default,
    more = TextStyle.Default,
    loading = TextStyle.Default,
    dragFill = Color.Blue,
    dragBorder = Color.Blue,
    pillFill = Color.Black,
    pillLabel = TextStyle.Default,
    camera = ColorPainter(Color.Black),
    repeat = ColorPainter(Color.Black),
)

/** One pixel to the dp, so a room in the tests reads the same in either unit. */
private val ONE = Density(1f)

@GraphicsMode(GraphicsMode.Mode.NATIVE)
@RunWith(RobolectricTestRunner::class)
class CalendarBlockPaintTest {

    private val ctx = RuntimeEnvironment.getApplication()
    private val measurer = TextMeasurer(createFontFamilyResolver(ctx), ONE, LayoutDirection.Ltr)

    private fun blocks(vararg segments: uniffi.mailcal_bindings.TimedSegment) =
        gridPage(timed = segments.toList())
            .toPaint(ctx, THEME, dark = false, use24Hour = true, locale = Locale.UK)
            .blocks

    private fun block(title: String, conference: Boolean = false, repeats: Boolean = false) = blocks(
        gridBlock(title = title, startMinutes = 9 * 60, endMinutes = 12 * 60).copy(
            hasConference = conference,
            occurrenceStart = if (repeats) "20260710T090000Z" else "",
        ),
    ).single()

    @Test
    fun a_block_wears_the_cores_event_colours_and_a_semibold_title() {
        val block = block("Design review")
        val swatch = testEventSwatch(GRID_WORK.color.hex, dark = false)
        assertEquals(parseHexColor(swatch.fill), block.fill)
        assertEquals(parseHexColor(swatch.accent), block.accent)
        assertEquals(parseHexColor(swatch.text), block.titleStyle.color)
        assertEquals(FontWeight.SemiBold, block.titleStyle.fontWeight)
        assertEquals("09:00–12:00", block.range)
    }

    @Test
    fun a_hold_is_a_fainter_tint_with_the_calendars_colour_at_its_edge() {
        val (hold, commitment) = blocks(
            gridBlock(title = "Hold").copy(participation = ResponseStatus.NEEDS_ACTION),
            gridBlock(title = "Commitment", day = 7),
        )
        assertTrue(hold.awaitingResponse)
        assertFalse(commitment.awaitingResponse)
        assertEquals(commitment.fill.alpha * HOLD_FILL_ALPHA, hold.fill.alpha, 0.001f)
        assertEquals(commitment.accent, hold.accent)
    }

    @Test
    fun an_occurrence_of_a_series_says_it_repeats() {
        val repeating = block("Standup", conference = true, repeats = true)
        assertTrue(repeating.repeats)
        assertEquals(2, repeating.cornerGlyphs)
        assertTrue(repeating.spoken, repeating.spoken.endsWith(", Video meeting, Repeats"))
        val once = block("Offsite")
        assertFalse(once.repeats)
        assertEquals(0, once.cornerGlyphs)
        assertFalse(once.spoken.contains("Repeats"))
    }

    @Test
    fun a_long_title_wraps_by_word_and_stops_at_the_last_whole_line() {
        val block = block("Quarterly planning review with the whole team")
        val label = with(ONE) { layOutBlockLabel(block, measurer, step = 10, lines = 2, withTime = true) }
        assertTrue(label.title.lineCount > 2)
        val ends = (0 until label.title.lineCount).map { label.title.getLineEnd(it) }
        assertFalse("a line broke a word at $ends", breaksAWord(block.title, ends))
        assertEquals(label.title.getLineBottom(1), label.titleHeight, 0.01f)
    }

    @Test
    fun a_word_wider_than_the_block_runs_on_as_one_line() {
        val block = block("Supercalifragilisticexpialidocious")
        val label = with(ONE) { layOutBlockLabel(block, measurer, step = 6, lines = 3, withTime = false) }
        assertEquals(1, label.title.lineCount)
        assertTrue(label.title.size.width > 48)
        assertEquals(label.title.getLineBottom(0), label.titleHeight, 0.01f)
    }

    @Test
    fun the_glyphs_are_drawn_only_while_the_title_keeps_36_dp() {
        val block = block("Standup", conference = true, repeats = true)
        // Two 11 dp glyphs and their gaps take 28 dp: 64 leaves 36 for the title, 56 leaves 28.
        val wide = with(ONE) { layOutBlockLabel(block, measurer, step = 8, lines = 1, withTime = false) }
        val narrow = with(ONE) { layOutBlockLabel(block, measurer, step = 7, lines = 1, withTime = false) }
        assertTrue(wide.glyphs)
        assertFalse(narrow.glyphs)
    }

    @Test
    fun the_time_is_a_range_where_it_fits_and_the_start_where_it_does_not() {
        val block = block("Design review")
        val wide = with(ONE) { layOutBlockLabel(block, measurer, step = 25, lines = 2, withTime = true) }
        val narrow = with(ONE) { layOutBlockLabel(block, measurer, step = 4, lines = 2, withTime = true) }
        assertEquals("09:00–12:00", wide.time?.layoutInput?.text?.text)
        assertEquals("09:00", narrow.time?.layoutInput?.text?.text)
    }

    @Test
    fun a_label_is_laid_out_once_per_room_and_line_count() {
        val labels = BlockLabels(1)
        val block = block("Design review")
        val label = with(ONE) { layOutBlockLabel(block, measurer, step = 10, lines = 2, withTime = true) }
        labels.hold(0, label)
        assertTrue(labels.held(0, step = 10, lines = 2, withTime = true) === label)
        assertEquals(null, labels.held(0, step = 10, lines = 3, withTime = true))
        assertEquals(null, labels.held(0, step = 11, lines = 2, withTime = true))
    }
}
