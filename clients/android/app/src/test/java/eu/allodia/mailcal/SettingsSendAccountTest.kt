// Settings → Composing's send account, drawn as the core decides it (`send_account_setting`).
//
// Which accounts are offered and which one is selected are the core's and tested there. What is
// tested here is that each variant is drawn as it is: no account says so, one account is shown with
// nothing to choose, and a choice shows the selected account and sends a pick back by its id.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.SendAccountOption
import uniffi.mailcal_bindings.SendAccountSetting

@RunWith(RobolectricTestRunner::class)
class SettingsSendAccountTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private fun picker() = compose.onNodeWithContentDescription(L10n.settings_send_account_heading(ctx()))

    @Test
    fun no_account_says_there_are_none() {
        compose.setContent { DefaultSendAccountCard(SendAccountSetting.NoAccount, onSelect = {}) }
        compose.onNodeWithText(L10n.settings_accounts_empty(ctx())).assertIsDisplayed()
        picker().assertDoesNotExist()
    }

    @Test
    fun one_account_shows_its_address_with_nothing_to_choose() {
        compose.setContent {
            DefaultSendAccountCard(SendAccountSetting.Only("me@work.example"), onSelect = {})
        }
        compose.onNodeWithText("me@work.example").assertIsDisplayed()
        picker().assertDoesNotExist()
    }

    @Test
    fun a_choice_shows_the_selected_account_and_sends_a_pick_by_its_id() {
        var picked: String? = null
        compose.setContent {
            DefaultSendAccountCard(
                SendAccountSetting.Choose(
                    accounts = listOf(
                        SendAccountOption(id = "acc-work", email = "me@work.example"),
                        SendAccountOption(id = "acc-home", email = "me@home.example"),
                    ),
                    selected = "acc-home",
                ),
                onSelect = { picked = it },
            )
        }
        // The core's selection is drawn, not the first account.
        compose.onNodeWithText("me@home.example").assertIsDisplayed()
        compose.onNodeWithText("me@work.example").assertDoesNotExist()

        picker().performClick()
        compose.onNodeWithText("me@work.example").performClick()
        assertEquals("acc-work", picked)
    }
}
