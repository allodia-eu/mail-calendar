// The month view's two client-side decisions: how it pages, and how many chips a cell can show.
//
// The core lays the month out and hands back every event on every day. What is decided here is what
// a *screen* can hold, and how it pages, because a month is the one view that a day-stride cannot
// express.
package eu.allodia.mailcal

import androidx.compose.ui.unit.dp
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import java.util.Locale
import org.junit.Rule
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.MonthCell
import uniffi.mailcal_bindings.MonthChip
import uniffi.mailcal_bindings.MonthPage
import uniffi.mailcal_bindings.ResponseStatus

class CalendarMonthTest {

    @Test
    fun a_month_pages_by_calendar_month_not_by_a_fixed_number_of_days() {
        // Months are 28–31 days long. Striding by any constant would drift off the month within a
        // year, silently, because each page still renders a perfectly plausible grid.
        val pager = CalendarPager(LocalDate.of(2026, 7, 15), CalendarMode.MONTH)
        assertEquals(LocalDate.of(2026, 7, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN))
        assertEquals(LocalDate.of(2026, 8, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN + 1))
        assertEquals(LocalDate.of(2026, 6, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN - 1))
        // Twelve pages on is exactly a year, whatever the month lengths in between.
        assertEquals(LocalDate.of(2027, 7, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN + 12))
    }

    @Test
    fun paging_from_the_end_of_a_long_month_does_not_lose_a_day_each_time() {
        // The trap: `plusMonths` from the 31st clamps to the 28th in February, and then the *next*
        // page is measured from the clamped date, so the anchor walks backwards through the year.
        // Anchoring on the 1st is what stops it.
        val pager = CalendarPager(LocalDate.of(2026, 1, 31), CalendarMode.MONTH)
        assertEquals(LocalDate.of(2026, 1, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN))
        assertEquals(LocalDate.of(2026, 2, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN + 1))
        assertEquals(LocalDate.of(2026, 3, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN + 2))
        assertEquals(LocalDate.of(2026, 4, 1), pager.anchorFor(CALENDAR_PAGE_ORIGIN + 3))
    }

    @Test
    fun the_month_is_not_a_time_grid() {
        assertTrue(CalendarMode.MONTH.isMonth)
        assertTrue(!CalendarMode.MONTH.isGrid)
        assertTrue(CalendarMode.WEEK.isGrid)
        assertTrue(!CalendarMode.WEEK.isMonth)
    }

    @Test
    fun a_cell_shows_everything_it_can_fit() {
        assertEquals(3, monthChipsShown(total = 3, capacity = 4))
        assertEquals(4, monthChipsShown(total = 4, capacity = 4))
        assertEquals(0, monthChipsShown(total = 0, capacity = 4))
    }

    @Test
    fun the_overflow_row_only_earns_its_place_when_it_stands_for_more_than_it_displaces() {
        // The subtlety. With capacity 4 and 5 events, drawing "+N more" COSTS a slot, so it would
        // draw 3 events and say "+2", hiding two to report two. Showing 3 + "+2" is right; showing
        // 4 and silently dropping one is not, and showing 3 + "+1" would be a lie.
        val capacity = 4
        val shown = monthChipsShown(total = 5, capacity = capacity)
        assertEquals(3, shown)
        assertEquals("the +N must count everything it is standing for", 2, 5 - shown)

        // And with a great many events, the count is still exact.
        assertEquals(3, monthChipsShown(total = 20, capacity = capacity))
        assertEquals(17, 20 - monthChipsShown(total = 20, capacity = capacity))
    }

    @Test
    fun a_cell_too_small_for_any_chip_draws_none_rather_than_a_sliver() {
        assertEquals(0, monthChipCapacity(0.dp))
        assertEquals(0, monthChipsShown(total = 3, capacity = 0))
        // A cell tall enough for four chips reports four.
        assertTrue(monthChipCapacity(60.dp) >= 4)
    }
}

// The month cell's chips, composed.
//
// A month chip is ~15dp tall, so the dashed border a grid block gets is most of the chip and the
// hatch is what actually reads. Neither is visible to a screen reader, which is why the assertion
// below is on what the chip *says*, docs/calendar.md §4, the spoken-grid rule. It is also the only
// half a test can see: the drawing is a `drawWithContent` on a canvas.
@RunWith(RobolectricTestRunner::class)
class MonthChipParticipationTest {
    @get:Rule val compose = createComposeRule()

    private fun chip(title: String, participation: ResponseStatus) = MonthChip(
        account = "acct-1",
        event = "evt-$title",
        calendar = "work",
        title = title,
        allDay = false,
        startMinutes = 600u,
        canWrite = true,
        participation = participation,
        // A layout case, not an identity one; the token is pinned where it is read.
        occurrenceStart = "",
    )

    private fun monthWith(chips: List<MonthChip>) = MonthPage(
        cells = (0 until 42).map { index ->
            MonthCell(
                date = LocalDate.of(2026, 6, 29).plusDays(index.toLong()).toString(),
                inMonth = true,
                chips = if (index == 0) chips else emptyList(),
            )
        },
        timezone = "Europe/Amsterdam",
        calendars = emptyList(),
        isMaterialized = true,
    )

    @Test
    fun an_unanswered_invitation_chip_says_it_is_awaiting_an_answer() {
        compose.setContent {
            AppTheme {
                CalendarMonthGrid(
                    page = monthWith(
                        listOf(
                            chip("Quarterly planning", ResponseStatus.NEEDS_ACTION),
                            chip("Design review", ResponseStatus.ACCEPTED),
                        ),
                    ),
                    today = LocalDate.of(2026, 7, 15),
                    locale = Locale.forLanguageTag("en-GB"),
                    weekStartsMonday = true,
                    onOpenEvent = { },
                )
            }
        }
        compose.onNodeWithContentDescription("Quarterly planning, Awaiting your response")
            .assertExists()
        // The commitment beside it keeps its plain title, a hold is marked, everything else is not.
        compose.onNodeWithContentDescription("Design review").assertExists()
    }
}
