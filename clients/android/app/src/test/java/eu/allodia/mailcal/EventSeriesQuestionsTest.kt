// The two questions a repeating event puts to the user on Android, and the one it must not.
//
// Both hang off the same fact, the occurrence the **core resolved** for the detail, so they are
// tested together: a write on ONE occurrence asks which occurrences it meant, a whole-series save
// that would discard the user's per-occurrence work says so before it writes, and an event with
// neither at stake gets no extra dialog at all. That last one is the load-bearing case: a dialog
// on every repeating event is what teaches people to click past the one that mattered.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.EventDetail
import uniffi.mailcal_bindings.SeriesEditWarning

private fun detail(
    isRecurring: Boolean = true,
    occurrence: String = "",
) = EventDetail(
    account = "acct",
    key = "/cal/e.ics",
    calendar = "work",
    title = "Standup",
    allDay = false,
    timezone = "Europe/Amsterdam",
    start = "2026-08-26T09:00:00",
    end = "2026-08-26T09:15:00",
    location = null,
    notes = null,
    reminderMinutes = null,
    recurrence = null,
    repeatSummary = null,
    repeatDraft = null,
    isRecurring = isRecurring,
    canWrite = true,
    occurrenceStart = occurrence,
    attendees = emptyList(),
)

@RunWith(RobolectricTestRunner::class)
class EventSeriesQuestionsTest {
    @get:Rule val compose = createComposeRule()

    private val ctx: android.content.Context = ApplicationProvider.getApplicationContext()

    // --- Which occurrences a delete meant -------------------------------------------------

    @Test
    fun a_block_the_core_named_an_occurrence_for_asks_about_the_series() {
        assertTrue(EventOpen("acct", "/cal/e.ics", "2026-09-02T09:00:00").asksAboutTheSeries)
    }

    @Test
    fun a_one_off_event_names_no_occurrence_and_asks_nothing() {
        assertFalse(EventOpen("acct", "/cal/e.ics", "").asksAboutTheSeries)
    }

    @Test
    fun opening_one_occurrence_skips_the_generic_confirm_and_goes_straight_to_the_question() {
        // The scope question carries its own way out, so raising the generic "Delete this event?"
        // first would make one delete cost two dialogs.
        var asked = 0
        compose.setContent {
            AppTheme {
                EventDetailScreen(
                    detail = detail(),
                    calendars = emptyList(),
                    onBack = {},
                    onEdit = {},
                    onDelete = { asked += 1 },
                    asksAboutTheSeries = true,
                )
            }
        }
        compose.onNodeWithText(L10n.action_delete(ctx)).performClick()
        assertEquals(1, asked)
        compose.onNodeWithText(L10n.event_delete_confirm(ctx)).assertDoesNotExist()
    }

    @Test
    fun opening_the_series_itself_still_gets_the_generic_confirm() {
        // An agenda row *is* the series: there is no occurrence to name, so nothing to ask about,
        // and the ordinary confirmation is the only thing standing between a tap and a delete.
        var asked = 0
        compose.setContent {
            AppTheme {
                EventDetailScreen(
                    detail = detail(),
                    calendars = emptyList(),
                    onBack = {},
                    onEdit = {},
                    onDelete = { asked += 1 },
                    asksAboutTheSeries = false,
                )
            }
        }
        compose.onNodeWithText(L10n.action_delete(ctx)).performClick()
        assertEquals(0, asked)
        compose.onNodeWithText(L10n.event_delete_confirm(ctx)).assertIsDisplayed()
    }

    // --- Which occurrences an edit meant ---------------------------------------------------

    @Test
    fun an_editor_opened_on_one_occurrence_asks_which_ones_the_save_meant() {
        val editor = EventEditorState.edit(detail(occurrence = "2026-09-09T09:00:00"), "Work")
        assertTrue(editor.asksAboutTheSeries)
    }

    @Test
    fun an_editor_opened_on_the_series_asks_nothing() {
        assertFalse(EventEditorState.edit(detail(occurrence = ""), "Work").asksAboutTheSeries)
    }

    @Test
    fun this_event_sends_the_occurrence_and_all_events_withholds_it() {
        // The whole scope question comes down to this one field, so both answers are asserted:
        // withholding it on *This event* rewrites every occurrence, and sending it on *All
        // events* splits an override instead of moving the series.
        val editor = EventEditorState.edit(detail(occurrence = "2026-09-09T09:00:00"), "Work")
        assertEquals(
            "2026-09-09T09:00:00",
            editor.updateArgs(thisOccurrenceOnly = true).occurrence,
        )
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).occurrence)
    }

    @Test
    fun an_editor_on_the_series_names_no_occurrence_either_way() {
        // Nothing to name, so even the answer that would send one cannot: an empty token would
        // have the core refuse a write that should have gone through.
        val editor = EventEditorState.edit(detail(occurrence = ""), "Work")
        assertNull(editor.updateArgs(thisOccurrenceOnly = true).occurrence)
        assertNull(editor.updateArgs(thisOccurrenceOnly = false).occurrence)
    }

    @Test
    fun both_answers_still_carry_both_edges() {
        // An occurrence's own times are not the series', so a single-occurrence edit naming
        // neither edge would move it onto the master's clock.
        val args = EventEditorState
            .edit(detail(occurrence = "2026-09-09T09:00:00"), "Work")
            .updateArgs(thisOccurrenceOnly = true)
        assertTrue(!args.start.isNullOrEmpty())
        assertTrue(!args.end.isNullOrEmpty())
    }

    // --- What a series edit would discard --------------------------------------------------

    @Test
    fun each_verdict_gets_its_own_sentence() {
        val texts = listOf(
            seriesWarningText(ctx, SeriesEditWarning.OCCURRENCES_RESET),
            seriesWarningText(ctx, SeriesEditWarning.RENAMES_SPREAD),
            seriesWarningText(ctx, SeriesEditWarning.OCCURRENCES_RESET_AND_RENAMES_SPREAD),
        )
        texts.forEach { assertTrue(!it.isNullOrEmpty()) }
        // A catalog key wired twice would say the wrong thing about the user's calendar, and
        // nothing on screen would tell the two apart.
        assertEquals(3, texts.toSet().size)
    }

    @Test
    fun nothing_to_say_is_no_sentence() {
        assertNull(seriesWarningText(ctx, null))
    }
}
