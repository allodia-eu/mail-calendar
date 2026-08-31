// The calendar grid's basic rendering, split out (with CalendarGridGeometryTest.kt,
// CalendarGridAllDayTest.kt and CalendarGridInvitationTest.kt) from what was one file: the block
// the core laid out is drawn, the header titles the period, the now line only shows on a page that
// contains today, and an unexpanded page says so rather than drawing a confidently empty week.
// The shared harness (driving the grid with synthetic pages, listening to it the way TalkBack
// does) is CalendarGridTestBase.kt.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.GridDay

@RunWith(RobolectricTestRunner::class)
class CalendarGridTest : CalendarGridTestBase() {

    @Test
    fun the_grid_draws_the_blocks_the_core_laid_out() {
        screen(gridPage())
        // The block is a coloured rectangle on a canvas, what it *says* is the only thing anyone who
        // cannot see it gets, and it names its calendar as well as its title and time.
        spoken("Team standup").assertExists()
        spoken("Work").assertExists()
    }

    @Test
    fun the_header_titles_the_period_and_numbers_the_week() {
        screen(gridPage())
        // The header chrome is still a composable, it is not part of the grid surface.
        compose.onNodeWithText("Jul 2026").assertIsDisplayed()
        // The ISO week number of 2026-07-06, drawn in the gutter and spoken from it.
        spoken("Week 28").assertExists()
    }

    @Test
    fun the_now_line_only_appears_on_a_page_that_actually_contains_today() {
        screen(gridPage())
        compose.onNodeWithContentDescription("Now").assertExists()
    }

    @Test
    fun a_page_without_today_draws_no_now_line() {
        // Paging away must hide the line, not draw it on an arbitrary Wednesday, a red line at
        // 09:45 across next month is a confident lie about where you are.
        val nextMonth = (3..9).map { GridDay("2026-08-%02d".format(it)) }
        screen(gridPage(days = nextMonth, timed = listOf(gridBlock(day = 0))))
        compose.onNodeWithContentDescription("Now").assertDoesNotExist()
    }

    @Test
    fun an_unexpanded_page_says_so_instead_of_drawing_a_confidently_empty_week() {
        // `isMaterialized = false` means "we have not looked this far yet", NOT "no events". An
        // empty grid here looks exactly like a real answer, which is the whole bug.
        screen(gridPage(timed = emptyList(), isMaterialized = false))
        spoken("Loading this period…").assertIsDisplayed()
    }

    @Test
    fun a_materialized_empty_week_is_a_real_answer_and_says_nothing() {
        screen(gridPage(timed = emptyList()))
        compose.onAllNodesWithContentDescription("Loading this period…").assertCountEquals(0)
    }
}
