// The invitee list as drawn in the event editor: offered where the calendar can host a meeting,
// explained where it cannot, and filled from the field. The decisions behind it are in
// EventInviteesTest; this holds that the screen shows them.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.core.app.ApplicationProvider
import java.time.LocalDateTime
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.InviteeCheck

private val NOW = LocalDateTime.of(2026, 8, 1, 9, 15, 0)

/** The core's check, reduced to what this screen needs: anything with an `@` is an address. */
private val check: InviteeCheckOf = { typed, _, _ ->
    if (typed.contains('@')) InviteeCheck.Valid(typed.trim()) else InviteeCheck.NotAnAddress
}

@RunWith(RobolectricTestRunner::class)
class EventInviteesScreenTest {
    @get:Rule val compose = createComposeRule()

    private val ctx: android.content.Context = ApplicationProvider.getApplicationContext()

    private fun editor(canInvite: Boolean) = EventEditorState.create(
        CalendarChoice("acct", "work", "Work", "me@example.org", canInvite),
        "Europe/Amsterdam",
        NOW,
        inviteePatchOf = { _, _ -> null },
        inviteeCheckOf = check,
    )

    private fun screen(editor: EventEditorState) {
        compose.setContent {
            EventEditorScreen(
                editor = editor,
                calendars = emptyList(),
                onCancel = {},
                onCreate = {},
                onUpdate = {},
                warningFor = { null },
            )
        }
        compose.waitForIdle()
    }

    @Test
    fun a_calendar_that_cannot_invite_says_why_and_offers_no_field() {
        screen(editor(canInvite = false))

        compose.onNodeWithText(L10n.event_invitees_unavailable(ctx)).assertExists()
        compose.onAllNodesWithText(L10n.event_invitee_add(ctx)).assertCountEquals(0)
    }

    @Test
    fun an_address_typed_and_invited_becomes_a_row_and_a_refusal_says_why() {
        val editor = editor(canInvite = true)
        screen(editor)
        val field = compose.onNodeWithTag("recipient-input-${L10n.event_invitee_field(ctx)}")

        field.performTextInput("grace@example.org")
        compose.onNodeWithText(L10n.event_invitee_add(ctx)).performClick()
        compose.onNodeWithText("grace@example.org").assertExists()
        compose.onNodeWithText(L10n.event_attendee_optional(ctx)).assertExists()

        field.performTextInput("nobody")
        compose.onNodeWithText(L10n.event_invitee_add(ctx)).performClick()
        compose.onNodeWithText(L10n.event_invitee_not_an_address(ctx)).assertExists()
        assert(editor.roster.invitees.map { it.email } == listOf("grace@example.org"))
    }
}
