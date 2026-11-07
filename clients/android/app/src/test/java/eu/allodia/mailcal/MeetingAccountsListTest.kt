// Settings → Videoconferencing's accounts that make video meetings (docs/settings.md category 4):
// drawn only while there is one, each saying its use's state and opening the account's page, with
// no switch of its own.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.isToggleable
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
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkedAccount
import uniffi.mailcal_bindings.MeetingAccount

@RunWith(RobolectricTestRunner::class)
class MeetingAccountsListTest {
    @get:Rule val compose = createComposeRule()

    private val ctx get() = ApplicationProvider.getApplicationContext<android.content.Context>()

    private val teams = MeetingAccount(
        LinkedAccount("alice@contoso.example@microsoft", "alice@contoso.example", null),
        "Microsoft Teams",
        CapabilityState.NEEDS_PERMISSION,
    )

    @Test
    fun a_meeting_account_says_its_use_in_the_words_its_page_would() {
        assertEquals(L10n.settings_toggle_on(ctx), meetingAccountState(ctx, CapabilityState.ON))
        assertEquals(L10n.settings_toggle_off(ctx), meetingAccountState(ctx, CapabilityState.OFF))
        assertEquals(
            L10n.settings_meeting_account_needs_permission(ctx),
            meetingAccountState(ctx, CapabilityState.NEEDS_PERMISSION),
        )
        assertEquals(
            L10n.settings_meeting_account_not_offered(ctx),
            meetingAccountState(ctx, CapabilityState.NOT_OFFERED),
        )
    }

    @Test
    fun no_account_that_makes_meetings_draws_no_card() {
        compose.setContent { MeetingAccountsCard(emptyList()) {} }
        compose.onAllNodesWithText(L10n.settings_meeting_accounts(ctx)).assertCountEquals(0)
    }

    @Test
    fun a_row_names_the_service_and_account_and_opens_its_page() {
        val opened = mutableListOf<String>()
        compose.setContent { MeetingAccountsCard(listOf(teams)) { opened += it } }
        compose.onNodeWithText(L10n.settings_meeting_accounts(ctx)).assertExists()
        compose.onNodeWithText("Microsoft Teams").assertExists()
        compose.onNodeWithText("alice@contoso.example").assertExists()
        compose.onNodeWithText(L10n.settings_meeting_account_needs_permission(ctx)).assertExists()
        // The switch is on the account's page, not here.
        compose.onAllNodes(isToggleable()).assertCountEquals(0)
        compose.onNodeWithTag("meeting-account-alice@contoso.example@microsoft").performClick()
        assertEquals(listOf("alice@contoso.example@microsoft"), opened)
    }
}
