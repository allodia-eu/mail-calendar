// Where the event editor's caret opens.
//
// The same rule the composer follows (docs/calendar.md, docs/contacts.md §4): a new event opens in
// its empty title, an edit does not, the event already has one, and raising the keyboard over the
// form hides the dates that are usually what the user came to change. It fails silently, which is
// why it is held here: the screen looks right in a picture whether or not the keyboard arrived.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.core.app.ApplicationProvider
import java.time.LocalDateTime
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.Swatch
import uniffi.mailcal_bindings.EventDetail

private val NOW = LocalDateTime.of(2026, 8, 1, 9, 15, 0)

private val WORK = CalendarChoice(account = "acct", id = "work", name = "Work")

private val CALENDARS = listOf(
    CalendarRow(
        account = "acct",
        id = "work",
        name = "Work",
        color = CalendarColor(
            hex = "#2f6fa8",
            light = Swatch("#2f6fa8", "#ffffff", "#245782"),
            dark = Swatch("#23537e", "#ffffff", "#2f6fa8"),
        ),
        visible = true,
        canWrite = true,
        isDefault = true,
    ),
)

private fun standup() = EventDetail(
    account = "acct",
    key = "/cal/e.ics",
    calendar = "work",
    title = "Standup",
    allDay = false,
    timezone = "Europe/Amsterdam",
    start = "2026-08-01T10:00:00",
    end = "2026-08-01T10:30:00",
    location = "Room 2",
    notes = "",
    reminderMinutes = null,
    recurrence = null,
    repeatSummary = null,
    repeatDraft = null,
    isRecurring = false,
    canWrite = true,
    occurrenceStart = "",
    attendees = emptyList(),
)

/// The same event as a series, opened either on the series itself or on one occurrence.
private fun recurring(occurrence: String) = standup().copy(
    isRecurring = true,
    occurrenceStart = occurrence,
)

@RunWith(RobolectricTestRunner::class)
class EventEditorFocusTest {
    @get:Rule val compose = createComposeRule()

    private val ctx: android.content.Context = ApplicationProvider.getApplicationContext()

    private fun screen(editor: EventEditorState) {
        compose.setContent {
            EventEditorScreen(
                editor = editor,
                calendars = CALENDARS,
                onCancel = {},
                onCreate = {},
                onUpdate = {},
                warningFor = { null },
            )
        }
        compose.waitForIdle()
    }

    @Test
    fun a_new_event_opens_with_the_caret_in_its_title() {
        screen(EventEditorState.create(WORK, "Europe/Amsterdam", NOW))

        compose.onNodeWithTag("event-title").assertIsFocused()
    }

    @Test
    fun editing_an_event_leaves_the_title_alone() {
        screen(EventEditorState.edit(standup(), "Work"))

        compose.onNodeWithTag("event-title").assertIsNotFocused()
    }

    // "Changes apply to the whole series." states one of the two answers the scope question is
    // about to ask for, so an editor that will ask may not also say it, the note and the
    // question hang off the same fact and can only disagree if one forgets to read it.
    //
    // Existence, not display: the note sits below the fold of a form that scrolls, so on the
    // test window it is composed and off-screen. Whether it is composed at all is the property.

    @Test
    fun an_editor_on_the_series_says_how_far_a_save_reaches() {
        screen(EventEditorState.edit(recurring(occurrence = ""), "Work"))

        compose.onNodeWithText(L10n.event_series_note(ctx)).assertExists()
    }

    @Test
    fun an_editor_on_one_occurrence_never_pre_empts_the_scope_question() {
        screen(EventEditorState.edit(recurring(occurrence = "2026-08-01T10:00:00"), "Work"))

        compose.onNodeWithText(L10n.event_series_note(ctx)).assertDoesNotExist()
    }
}
