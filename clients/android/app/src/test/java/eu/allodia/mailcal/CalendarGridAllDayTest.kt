// The all-day banner above the grid: it caps itself and says what it hid, an exact fit offers no
// expand, and a multi-day band spans its columns. Split out of CalendarGridTest.kt.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AllDayBand
import uniffi.mailcal_bindings.ResponseStatus

@RunWith(RobolectricTestRunner::class)
class CalendarGridAllDayTest : CalendarGridTestBase() {

    @Test
    fun a_crowded_all_day_banner_caps_itself_and_says_what_it_hid() {
        // Five lanes is more than the collapsed banner shows. It must not simply grow, an uncapped
        // banner swallows the grid it sits above, and it must not silently drop the rest either.
        val bands = (0 until 5).map { lane ->
            AllDayBand(
                account = "acct-1",
                event = "evt-$lane",
                calendar = "work",
                title = "Band $lane",
                day = 0u,
                days = 7u,
                lane = lane.toUInt(),
                continuesBefore = false,
                continuesAfter = false,
                canWrite = true,
                participation = ResponseStatus.ACCEPTED,
                // A layout case, not an identity one; the token is pinned where it is read.
                occurrenceStart = "",
            )
        }
        screen(gridPage(allDay = bands, allDayLanes = 5u))

        // Two rows of bars, then a row of "+3" chips, the three lanes it is holding back.
        spoken("Band 0").assertExists()
        spoken("Band 1").assertExists()
        compose.onAllNodesWithContentDescription("Band 4", substring = true).assertCountEquals(0)

        // **One chip per day column, and every one of them says "3".** These five bands each span the
        // whole week, so all seven columns are hiding all three of the lanes past the cap. A single
        // global "+N" would be wrong on six of them, and a "+1" that should say "+3" is a lie the
        // user cannot see through: they tap, find events nobody told them about, and stop trusting
        // the banner.
        val chips = compose.onAllNodesWithContentDescription("Show 3 more all-day events")
        chips.assertCountEquals(DAYS_IN_WEEK)

        // Tapping the banner reveals them, and the "+N" goes away rather than lingering as a lie.
        // The tap lands on the canvas: the node over the chip carries no click of its own, and the
        // single gesture owner underneath is what decides that a tap on the banner means "expand".
        chips.onFirst().performClick()
        spoken("Band 4").assertExists()
        compose
            .onAllNodesWithContentDescription("more all-day events", substring = true)
            .assertCountEquals(0)
    }

    @Test
    fun an_all_day_banner_that_fits_offers_no_expand() {
        // Exactly three lanes fit. A "+N" here would be hiding an event for no reason at all.
        val bands = (0 until 3).map { lane ->
            AllDayBand(
                account = "acct-1",
                event = "evt-$lane",
                calendar = "work",
                title = "Band $lane",
                day = 0u,
                days = 7u,
                lane = lane.toUInt(),
                continuesBefore = false,
                continuesAfter = false,
                canWrite = true,
                participation = ResponseStatus.ACCEPTED,
                occurrenceStart = "",
            )
        }
        screen(gridPage(allDay = bands, allDayLanes = 3u))
        spoken("Band 2").assertExists()
        compose.onAllNodesWithContentDescription("more all-day", substring = true)
            .assertCountEquals(0)
    }

    @Test
    fun a_multi_day_banner_spans_its_columns_above_the_grid() {
        val band = AllDayBand(
            account = "acct-1",
            event = "evt-offsite",
            calendar = "work",
            title = "Offsite",
            day = 1u,
            days = 3u,
            lane = 0u,
            continuesBefore = false,
            continuesAfter = false,
            canWrite = true,
            participation = ResponseStatus.ACCEPTED,
            occurrenceStart = "",
        )
        // A single-day bar beside it, in the same lane, as the unit of measurement.
        val oneDay = AllDayBand(
            account = "acct-1",
            event = "evt-dentist",
            calendar = "work",
            title = "Dentist",
            day = 5u,
            days = 1u,
            lane = 0u,
            continuesBefore = false,
            continuesAfter = false,
            canWrite = true,
            participation = ResponseStatus.ACCEPTED,
            occurrenceStart = "",
        )
        screen(gridPage(allDay = listOf(band, oneDay), allDayLanes = 1u))
        spoken("Offsite").assertIsDisplayed()

        // Three days from Tuesday is three columns wide, a multi-day bar spans its days, and does
        // not sit as a chip on the first of them.
        val offsite = boundsOf("Offsite")
        val dentist = boundsOf("Dentist")
        assertEquals(
            "a three-day band must be three columns wide",
            (dentist.right - dentist.left).value * 3f,
            (offsite.right - offsite.left).value,
            1.5f,
        )
        assertTrue("it starts on its own day, not the first of the week", offsite.left > 0.dp)
        // And it says it is an all-day event, not a timed one starting at midnight.
        spoken("Offsite, All day").assertExists()
    }
}
