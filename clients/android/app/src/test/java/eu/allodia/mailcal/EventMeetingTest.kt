// What the detail says about a video meeting, and what its links open (docs/conferencing.md). The
// links are the part that matters most: Join and a dial-in number are the two places a tap leaves
// the app, so what each will open is pinned here, and the section is driven to show that a tap
// hands over exactly that.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.ConferenceDialIn
import uniffi.mailcal_bindings.ConferenceState
import uniffi.mailcal_bindings.EventConference

private const val TEAMS = "https://teams.microsoft.com/meet/123?p=x"

private fun meeting(
    service: String? = "Microsoft Teams",
    joinUrl: String? = TEAMS,
    meetingId: String? = "123 456",
    passcode: String? = null,
    dialIn: List<ConferenceDialIn> = emptyList(),
    state: ConferenceState = ConferenceState.READY,
) = EventConference(
    service = service,
    joinUrl = joinUrl,
    meetingId = meetingId,
    passcode = passcode,
    dialIn = dialIn,
    notes = null,
    state = state,
)

// The core's link gate, as far as these links need it: the JVM suite cannot load the core.
private val webOnly: (String) -> Boolean = { it.startsWith("https:") || it.startsWith("http:") }

@RunWith(RobolectricTestRunner::class)
class EventMeetingTest {
    @get:Rule val compose = createComposeRule()

    private val ctx get() = RuntimeEnvironment.getApplication()

    @Test
    fun a_meeting_is_headed_by_its_service_and_otherwise_by_the_catalogs_words() {
        assertEquals("Microsoft Teams", EventMeeting.title(meeting(), "Video meeting"))
        assertEquals("Video meeting", EventMeeting.title(meeting(service = null), "Video meeting"))
    }

    @Test
    fun join_opens_the_cores_web_link_and_nothing_else() {
        assertEquals(TEAMS, EventMeeting.joinLink(meeting(), webOnly))
        assertNull(EventMeeting.joinLink(meeting(joinUrl = null), webOnly))
        assertNull(EventMeeting.joinLink(meeting(joinUrl = "javascript:alert(1)"), { true }))
        assertNull("the gate refused it", EventMeeting.joinLink(meeting(), { false }))
        assertNull(
            "plain http is not a join link",
            EventMeeting.joinLink(meeting(joinUrl = "http://teams.microsoft.com/meet/1"), webOnly),
        )
    }

    @Test
    fun a_meeting_still_being_made_says_so_instead_of_offering_join() {
        val preparing = meeting(state = ConferenceState.PREPARING)
        assertNull(EventMeeting.joinLink(preparing, webOnly))
        assertNull(EventMeeting.status(meeting(), "preparing", "failed"))
        assertEquals("preparing", EventMeeting.status(preparing, "preparing", "failed"))
        assertEquals(
            "failed",
            EventMeeting.status(meeting(state = ConferenceState.FAILED), "preparing", "failed"),
        )
    }

    @Test
    fun a_dial_in_row_opens_only_a_telephone_number() {
        val number = ConferenceDialIn(number = "+31 20 555 0100", uri = "tel:+31205550100", pin = "990#")
        assertEquals("tel:+31205550100", EventMeeting.dialInLink(number))
        assertNull(EventMeeting.dialInLink(ConferenceDialIn("x", "https://example.com/", null)))
    }

    @Test
    fun a_month_chip_says_its_hold_and_its_meeting() {
        val meeting = L10n.event_video_meeting(ctx)
        val hold = L10n.a11y_invitation_awaiting_response(ctx)
        assertEquals("Sync", monthChipLabel(ctx, "Sync", awaiting = false, hasConference = false))
        assertEquals("Sync, $meeting", monthChipLabel(ctx, "Sync", awaiting = false, hasConference = true))
        assertEquals(
            "Sync, $hold, $meeting",
            monthChipLabel(ctx, "Sync", awaiting = true, hasConference = true),
        )
    }

    @Test
    fun join_copy_and_a_number_hand_over_exactly_what_they_show() {
        val joined = mutableListOf<String>()
        val dialled = mutableListOf<String>()
        val copied = mutableListOf<String>()
        val actions = MeetingActions(
            join = { joined += it },
            dial = { dialled += it },
            copy = { copied += it },
            mayOpen = webOnly,
        )
        val conference = meeting(
            passcode = "x7Y",
            dialIn = listOf(
                ConferenceDialIn("+31 20 555 0100", "tel:+31205550100", "990#"),
                // A server can list one number twice with a different PIN: two rows.
                ConferenceDialIn("+31 20 555 0100", "tel:+31205550100", "991#"),
            ),
        )
        compose.setContent { EventConferenceSection(conference, 0, actions) }

        compose.onNodeWithTag("event-join").performClick()
        compose.onNodeWithTag("event-meeting-copy-link").performClick()
        compose.onAllNodesWithText("+31 20 555 0100").assertCountEquals(2)
        compose.onAllNodesWithText("+31 20 555 0100")[0].performClick()
        compose.onNodeWithText(L10n.event_meeting_pin(ctx, "991#")).assertExists()
        compose.onNodeWithTag("event-meeting-passcode").assertExists()

        assertEquals(listOf(TEAMS), joined)
        assertEquals(listOf(TEAMS), copied)
        assertEquals(listOf("tel:+31205550100"), dialled)
    }

    @Test
    fun a_meeting_with_nothing_to_join_offers_nothing_to_tap_and_leaves_empty_rows_out() {
        val actions = MeetingActions(join = {}, dial = {}, copy = {}, mayOpen = webOnly)
        val conference = meeting(joinUrl = null, meetingId = "", state = ConferenceState.PREPARING)
        compose.setContent { EventConferenceSection(conference, 0, actions) }

        compose.onNodeWithText(L10n.event_meeting_preparing(ctx)).assertExists()
        compose.onAllNodesWithTag("event-join").assertCountEquals(0)
        compose.onAllNodesWithTag("event-meeting-copy-link").assertCountEquals(0)
        compose.onAllNodesWithTag("event-meeting-id").assertCountEquals(0)
    }
}
