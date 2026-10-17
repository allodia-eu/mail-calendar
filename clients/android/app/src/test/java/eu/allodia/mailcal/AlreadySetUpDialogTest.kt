// The "already set up" refusal's offer (docs/accounts.md rules 15 and 17): it names what updating
// keeps, updates only when pressed, and holds still while the account takes the credential.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class AlreadySetUpDialogTest {
    @get:Rule val compose = createComposeRule()

    private var updates = 0
    private var dismissed = 0
    private var updating by mutableStateOf(false)

    private fun show() {
        compose.setContent {
            AlreadySetUpDialog(updating = updating, onUpdate = { updates++ }, onDismiss = { dismissed++ })
        }
        compose.waitForIdle()
    }

    @Test
    fun the_refusal_offers_to_update_the_account_already_there() {
        show()

        compose.onNodeWithText("This account is already set up.").assertExists()
        compose.onNodeWithText("Its mail and settings stay as they are.", substring = true).assertExists()
        compose.onNodeWithText("Update the existing account").performClick()

        assertEquals(1, updates)
        assertEquals(0, dismissed)
    }

    @Test
    fun cancel_leaves_the_account_as_it_was() {
        show()

        compose.onNodeWithText("Cancel").performClick()

        assertEquals(0, updates)
        assertEquals(1, dismissed)
    }

    @Test
    fun an_update_under_way_cannot_be_pressed_again() {
        updating = true
        show()

        compose.onNodeWithTag("setup-update-existing").assertIsNotEnabled()
        compose.onNodeWithText("Cancel").assertIsNotEnabled()
    }
}
