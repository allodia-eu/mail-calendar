// Settings → Advanced, drawn as the core decides it (`advanced_page`).
//
// When a reset runs and what it came to are the core's and tested there. What is tested here is
// that each state is drawn as it is: a free button asks, a running one shows the spinner and cannot
// be pressed, and each outcome is said in its own words.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
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
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AdvancedPage
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.ResetOutcome

/** Settings → Advanced with no reset running and none made yet. */
internal val ADVANCED_FREE = AdvancedPage(assistants = null, reset = FREE, resetOutcome = null)

@RunWith(RobolectricTestRunner::class)
class SettingsAdvancedTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private var asked = 0

    private fun show(running: Boolean, outcome: ResetOutcome? = null) {
        val page = AdvancedPage(
            assistants = null,
            reset = ControlState(enabled = !running, running = running),
            resetOutcome = outcome,
        )
        compose.setContent { AdvancedSettings(page, onRequestReset = { asked++ }) }
    }

    @Test
    fun a_free_reset_asks_first_and_says_nothing_yet() {
        show(running = false)
        compose.onNodeWithTag(RESET_BUTTON).assertIsEnabled().performClick()
        assertEquals(1, asked)
        compose.onNodeWithTag("$RESET_BUTTON-running", useUnmergedTree = true).assertDoesNotExist()
        compose.onNodeWithTag(RESET_OUTCOME).assertDoesNotExist()
    }

    @Test
    fun a_running_reset_shows_the_spinner_and_cannot_be_pressed() {
        show(running = true)
        compose.onNodeWithTag("$RESET_BUTTON-running", useUnmergedTree = true).assertExists()
        compose.onNodeWithTag(RESET_BUTTON).assertIsNotEnabled().performClick()
        assertEquals(0, asked)
        compose.onNodeWithTag(RESET_OUTCOME).assertDoesNotExist()
    }

    @Test
    fun a_reset_that_worked_says_so() {
        show(running = false, outcome = ResetOutcome.DONE)
        compose.onNodeWithText(L10n.settings_advanced_reset_done(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_advanced_reset_failed(ctx())).assertDoesNotExist()
        compose.onNodeWithTag(RESET_BUTTON).assertIsEnabled()
    }

    @Test
    fun a_reset_that_failed_says_so() {
        show(running = false, outcome = ResetOutcome.FAILED)
        compose.onNodeWithText(L10n.settings_advanced_reset_failed(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.settings_advanced_reset_done(ctx())).assertDoesNotExist()
    }
}
