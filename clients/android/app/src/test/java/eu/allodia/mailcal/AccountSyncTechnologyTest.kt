// What a Gmail account's page says about which way its mail syncs (docs/settings.md row 9): each
// technology its own word and explanation, drawn on the card, with the explanation behind the
// info button.
package eu.allodia.mailcal

import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.MailSyncTechnology

@RunWith(RobolectricTestRunner::class)
class AccountSyncTechnologyTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    @Test
    fun eachTechnologyHasAWordAndAnExplanationOfItsOwn() {
        val all = MailSyncTechnology.entries
        val names = all.map { AccountSyncTechnologyLabels.name(ctx(), it) }
        val details = all.map { AccountSyncTechnologyLabels.detail(ctx(), it) }
        assertEquals(all.size, names.toSet().size)
        assertEquals(all.size, details.toSet().size)
    }

    @Test
    fun theInfoButtonOpensTheExplanation() {
        compose.setContent { AccountSyncTechnologyCard(MailSyncTechnology.FALLBACK) }
        compose.onNodeWithTag("account-sync-technology")
            .assertTextEquals(L10n.settings_account_sync_technology_fallback(ctx()))
        compose.onNodeWithContentDescription(L10n.settings_account_sync_technology_about(ctx()))
            .performClick()
        compose.onNodeWithText(L10n.settings_account_sync_technology_fallback_detail(ctx()))
            .assertExists()
        compose.onNodeWithText(L10n.settings_account_sync_technology_learn_more(ctx()))
            .assertExists()
    }
}
