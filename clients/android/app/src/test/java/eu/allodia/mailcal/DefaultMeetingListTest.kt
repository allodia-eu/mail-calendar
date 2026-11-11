// Settings → Videoconferencing's default per account (docs/conferencing.md, rule 10): drawn only
// while an account's calendars offer a meeting, each row saying what is chosen and reporting a
// pick with its account.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.MeetingDefault
import uniffi.mailcal_bindings.MeetingOption

@RunWith(RobolectricTestRunner::class)
class DefaultMeetingListTest {
    @get:Rule val compose = createComposeRule()

    private val ctx get() = ApplicationProvider.getApplicationContext<android.content.Context>()

    private val talk = MeetingOption("account:talk", "Nextcloud Talk", "alice@cloud.test.local")
    private val jitsi = MeetingOption("jitsi", "Jitsi Meet")

    private fun row(chosen: String?) =
        MeetingDefault("alice@dav", "alice@work.test.local", listOf(talk, jitsi), chosen)

    @Test
    fun no_account_that_offers_a_meeting_draws_no_card() {
        compose.setContent { DefaultMeetingCard(emptyList()) { _, _ -> } }
        compose.onAllNodesWithText(L10n.settings_default_meeting(ctx)).assertCountEquals(0)
    }

    @Test
    fun a_row_names_the_account_and_what_is_chosen() {
        compose.setContent { DefaultMeetingCard(listOf(row("account:talk"))) { _, _ -> } }
        compose.onNodeWithText(L10n.settings_default_meeting(ctx)).assertExists()
        compose.onNodeWithText("alice@work.test.local").assertExists()
        compose.onNodeWithText(
            L10n.event_meeting_through(ctx, "Nextcloud Talk", "alice@cloud.test.local"),
            useUnmergedTree = true,
        ).assertExists()
    }

    @Test
    fun a_row_with_nothing_chosen_says_none() {
        compose.setContent { DefaultMeetingCard(listOf(row(null))) { _, _ -> } }
        compose.onNodeWithText(L10n.event_meeting_none(ctx), useUnmergedTree = true).assertExists()
    }

    @Test
    fun a_pick_reports_its_account_and_none_reports_no_choice() {
        val picked = mutableListOf<Pair<String, String?>>()
        compose.setContent {
            DefaultMeetingCard(listOf(row("jitsi"))) { account, choice -> picked += account to choice }
        }
        compose.onNodeWithTag("settings-default-meeting-alice@dav").performClick()
        compose.onNodeWithText(L10n.event_meeting_through(ctx, "Nextcloud Talk", "alice@cloud.test.local"))
            .performClick()
        compose.onNodeWithTag("settings-default-meeting-alice@dav").performClick()
        compose.onNodeWithText(L10n.event_meeting_none(ctx)).performClick()
        assertEquals(listOf<Pair<String, String?>>("alice@dav" to "account:talk", "alice@dav" to null), picked)
    }
}
