// The separate server's form with its sign-in in the browser (docs/mail-oauth.md rule 13), drawn
// from the core's section: above the fields only once the core offers it, with its note beneath
// and the password kept as the alternative; running as the core says; while the browser holds
// it, a line saying so with its Cancel, a fresh press still taken; and leaving the screen cancels
// a sign-in still out.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountSeparateServer
import uniffi.mailcal_bindings.AccountSeparateSignIn
import uniffi.mailcal_bindings.BrowserSignIn
import uniffi.mailcal_bindings.ControlState

@RunWith(RobolectricTestRunner::class)
class SeparateServerSignInScreenTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx(): Context = RuntimeEnvironment.getApplication()

    private var section by mutableStateOf(separateSection(AccountCapability.CALENDAR))
    private var showing by mutableStateOf(true)
    private var signIns = 0
    private var skips = 0
    private var dismissed = 0
    private val cancelled = mutableListOf<BrowserSignIn>()

    private fun offered(signInControl: ControlState = FREE): AccountSeparateServer = separateSection(
        AccountCapability.CALENDAR,
        signIn = AccountSeparateSignIn.PROVIDER,
        signInControl = signInControl,
    )

    private fun step() {
        compose.setContent {
            if (showing) {
                val form = separateForm(section)
                SeparateServerStepScreen(
                    form,
                    SeparateControls.of(form.section),
                    null,
                    { _, _ -> },
                    { skips++ },
                    onSignIn = { signIns++ },
                    onCancelSignIn = { cancelled += it },
                )
            }
        }
        compose.waitForIdle()
    }

    private fun dialog() {
        compose.setContent {
            val form = separateForm(section)
            SeparateServerDialog(
                form,
                SeparateControls.of(form.section),
                null,
                { _, _ -> },
                { dismissed++ },
                onSignIn = { signIns++ },
                onCancelSignIn = { cancelled += it },
            )
        }
        compose.waitForIdle()
    }

    @Test
    fun nothing_is_offered_until_the_core_does() {
        step()

        compose.onNodeWithTag("separate-server-sign-in").assertDoesNotExist()
        compose.onNodeWithText(L10n.setup_imap_signin_note(ctx())).assertDoesNotExist()
    }

    @Test
    fun an_offered_sign_in_sits_above_the_fields_with_its_note_beneath() {
        section = offered()
        step()

        val button = compose.onNodeWithTag("separate-server-sign-in").assertIsEnabled()
        compose.onNodeWithText(L10n.setup_imap_signin_button(ctx())).assertExists()
        val buttonBounds = button.getUnclippedBoundsInRoot()
        val note = compose.onNodeWithText(L10n.setup_imap_signin_note(ctx())).getUnclippedBoundsInRoot()
        val username = compose.onNodeWithText(L10n.setup_field_username(ctx())).getUnclippedBoundsInRoot()
        assertTrue(note.top >= buttonBounds.bottom)
        assertTrue(username.top >= note.bottom)
        compose.onNodeWithText(L10n.setup_field_password(ctx())).assertExists()
        compose.onNodeWithTag("separate-server-sign-in-cancel").assertDoesNotExist()

        button.performClick()
        assertEquals(1, signIns)
    }

    @Test
    fun the_answer_arriving_keeps_the_field_being_typed_in() {
        step()
        compose.onNodeWithText(L10n.setup_field_password(ctx())).performTextInput("secr")

        section = offered()
        compose.waitForIdle()

        compose.onNodeWithTag("separate-server-sign-in").assertExists()
        compose.onNodeWithText(L10n.setup_field_password(ctx())).assertIsFocused()
        compose.onNodeWithTag("separate-server-connect").assertIsEnabled()
    }

    @Test
    fun while_the_core_starts_it_the_button_runs() {
        section = offered(ControlState(enabled = false, running = true))
        step()

        compose.onNodeWithTag("separate-server-sign-in").assertIsNotEnabled()
        compose.onNodeWithTag("separate-server-sign-in-running").assertExists()
        compose.onNodeWithTag("separate-server-skip").assertIsEnabled().performClick()

        assertEquals(1, skips)
    }

    @Test
    fun while_the_browser_has_it_the_line_says_so_with_its_cancel_and_a_fresh_press_is_taken() {
        section = separateWaiting(AccountCapability.CALENDAR)
        step()

        compose.onNodeWithText(L10n.settings_account_signin_waiting(ctx(), "alice")).assertExists()
        compose.onNodeWithText(L10n.setup_imap_signin_note(ctx())).assertDoesNotExist()
        compose.onNodeWithTag("separate-server-connect").assertIsNotEnabled()
        compose.onNodeWithTag("separate-server-skip").assertIsNotEnabled()
        compose.onNodeWithTag("separate-server-sign-in").assertIsEnabled().performClick()
        assertEquals(1, signIns)

        compose.onNodeWithTag("separate-server-sign-in-cancel").assertIsEnabled().performClick()
        assertEquals(listOf(4uL), cancelled.map { it.attempt })
    }

    @Test
    fun leaving_the_screen_cancels_the_sign_in_still_out_once() {
        section = separateWaiting(AccountCapability.CALENDAR)
        step()

        showing = false
        compose.waitForIdle()

        assertEquals(listOf(4uL), cancelled.map { it.attempt })
    }

    @Test
    fun leaving_the_screen_with_nothing_out_cancels_nothing() {
        section = offered()
        step()

        showing = false
        compose.waitForIdle()

        assertTrue(cancelled.isEmpty())
    }

    @Test
    fun the_page_dialog_offers_it_and_keeps_cancel_while_the_browser_has_it() {
        section = offered()
        dialog()
        compose.onNodeWithTag("separate-server-sign-in").performClick()
        assertEquals(1, signIns)

        section = separateWaiting(AccountCapability.CALENDAR)
        compose.waitForIdle()
        compose.onNodeWithTag("separate-server-sign-in-cancel").assertExists()
        compose.onNodeWithTag("separate-server-cancel").assertIsEnabled().performClick()

        assertEquals(1, dismissed)
    }
}
