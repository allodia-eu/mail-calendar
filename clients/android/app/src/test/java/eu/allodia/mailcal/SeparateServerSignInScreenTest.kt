// The separate server's form with the sign-in at the server's own provider (docs/mail-oauth.md
// rule 13): drawn above the fields only once the core offers it, with its note beneath, the
// password kept as the alternative, and the way back open while nothing blocks.
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
import uniffi.mailcal_bindings.AccountSeparateSignIn

@RunWith(RobolectricTestRunner::class)
class SeparateServerSignInScreenTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx(): Context = RuntimeEnvironment.getApplication()

    private val form = separateForm(separateSection(AccountCapability.CALENDAR))
    private var controls by mutableStateOf(SeparateControls.of(form.section, null))
    private var showing by mutableStateOf(true)
    private var signIns = 0
    private var skips = 0
    private var dismissed = 0
    private var left = 0

    private fun offered(step: SeparateSignInStep? = null) = SeparateControls.of(
        separateSection(AccountCapability.CALENDAR, signIn = AccountSeparateSignIn.PROVIDER),
        step,
    )

    private fun step() {
        compose.setContent {
            if (showing) {
                SeparateServerStepScreen(
                    form,
                    controls,
                    null,
                    { _, _ -> },
                    { skips++ },
                    onSignIn = { signIns++ },
                    onLeave = { left++ },
                )
            }
        }
        compose.waitForIdle()
    }

    private fun dialog() {
        compose.setContent {
            SeparateServerDialog(
                form,
                controls,
                null,
                { _, _ -> },
                { dismissed++ },
                onSignIn = { signIns++ },
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
        controls = offered()
        step()

        val button = compose.onNodeWithTag("separate-server-sign-in").assertIsEnabled()
        compose.onNodeWithText(L10n.setup_imap_signin_button(ctx())).assertExists()
        val buttonBounds = button.getUnclippedBoundsInRoot()
        val note = compose.onNodeWithText(L10n.setup_imap_signin_note(ctx())).getUnclippedBoundsInRoot()
        val username = compose.onNodeWithText(L10n.setup_field_username(ctx())).getUnclippedBoundsInRoot()
        assertTrue(note.top >= buttonBounds.bottom)
        assertTrue(username.top >= note.bottom)
        compose.onNodeWithText(L10n.setup_field_password(ctx())).assertExists()

        button.performClick()
        assertEquals(1, signIns)
    }

    @Test
    fun the_answer_arriving_keeps_the_field_being_typed_in() {
        step()
        compose.onNodeWithText(L10n.setup_field_password(ctx())).performTextInput("secr")

        controls = offered()
        compose.waitForIdle()

        compose.onNodeWithTag("separate-server-sign-in").assertExists()
        compose.onNodeWithText(L10n.setup_field_password(ctx())).assertIsFocused()
        compose.onNodeWithTag("separate-server-connect").assertIsEnabled()
    }

    @Test
    fun while_it_starts_only_the_way_back_stays() {
        controls = offered(SeparateSignInStep.STARTING)
        step()
        compose.onNodeWithText(L10n.setup_field_password(ctx())).performTextInput("secret")

        compose.onNodeWithTag("separate-server-sign-in").assertIsNotEnabled()
        compose.onNodeWithTag("separate-server-connect").assertIsNotEnabled()
        compose.onNodeWithTag("separate-server-skip").assertIsEnabled().performClick()

        assertEquals(1, skips)
    }

    @Test
    fun while_the_browser_has_it_a_fresh_press_is_taken() {
        controls = offered(SeparateSignInStep.BROWSER)
        step()

        compose.onNodeWithTag("separate-server-sign-in").assertIsEnabled().performClick()

        assertEquals(1, signIns)
    }

    @Test
    fun while_it_completes_everything_holds_still() {
        controls = offered(SeparateSignInStep.COMPLETING)
        step()

        compose.onNodeWithTag("separate-server-sign-in").assertIsNotEnabled()
        compose.onNodeWithTag("separate-server-skip").assertIsNotEnabled()
    }

    @Test
    fun leaving_the_screen_says_so_once() {
        step()

        showing = false
        compose.waitForIdle()

        assertEquals(1, left)
    }

    @Test
    fun the_page_dialog_offers_it_and_keeps_cancel_while_it_starts() {
        controls = offered()
        dialog()
        compose.onNodeWithTag("separate-server-sign-in").performClick()
        assertEquals(1, signIns)

        controls = offered(SeparateSignInStep.STARTING)
        compose.waitForIdle()
        compose.onNodeWithTag("separate-server-cancel").assertIsEnabled().performClick()

        assertEquals(1, dismissed)
    }
}
