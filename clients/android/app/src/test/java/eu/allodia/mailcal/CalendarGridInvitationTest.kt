// The unanswered-invitation tests split out of CalendarGridTest.kt (docs/invitations.md).
package eu.allodia.mailcal

import androidx.compose.ui.test.onAllNodesWithContentDescription
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AllDayBand
import uniffi.mailcal_bindings.ResponseStatus

@RunWith(RobolectricTestRunner::class)
class CalendarGridInvitationTest : CalendarGridTestBase() {

    // ---- Unanswered holds (docs/invitations.md) ---------------------------------------------------

    @Test
    fun an_unanswered_invitation_says_so_and_a_commitment_does_not() {
        // The dashed border and the hatched gutter are *drawn*, so they are invisible to exactly the
        // user this rule exists for. What a screen reader gets is the whole disclosure
        // (docs/calendar.md §4), which makes this the assertion that matters, and it runs through
        // the real page builder, so it fails if `toPaint` ever stops calling `calendarEventLabel`.
        screen(
            gridPage(
                timed = listOf(
                    gridBlock(title = "Quarterly planning").copy(
                        participation = ResponseStatus.NEEDS_ACTION,
                    ),
                    gridBlock(title = "Design review", day = 7),
                ),
            ),
        )
        spoken("Quarterly planning, 09:30 – 10:30, Work, Awaiting your response").assertExists()
        // And the commitment beside it is not relabelled, a hold is told apart by being marked,
        // never by everything else being marked differently.
        assertEquals(
            0,
            compose.onAllNodesWithContentDescription(
                "Design review, 09:30 – 10:30, Work, Awaiting your response",
            ).fetchSemanticsNodes().size,
        )
    }

    @Test
    fun an_unanswered_all_day_invitation_says_so_too() {
        // The all-day banner is a different draw path from the grid, so it is a different chance to
        // forget the label.
        screen(
            gridPage(
                timed = emptyList(),
                allDay = listOf(
                    AllDayBand(
                        account = "acct-1",
                        event = "evt-summit",
                        calendar = "work",
                        title = "Summit",
                        day = 1u,
                        days = 1u,
                        lane = 0u,
                        continuesBefore = false,
                        continuesAfter = false,
                        canWrite = true,
                        participation = ResponseStatus.NEEDS_ACTION,
                        occurrenceStart = "",
                    ),
                ),
                allDayLanes = 1u,
            ),
        )
        spoken("Summit, All day, Work, Awaiting your response").assertExists()
    }
}
