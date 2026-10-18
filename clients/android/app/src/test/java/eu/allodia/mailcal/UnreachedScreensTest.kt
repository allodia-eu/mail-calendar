// What is drawn for a calendar or address book a sign-in does not reach (docs/accounts.md rule 18):
// the question asked before the browser opens, with its three answers, and the form that gives
// them a sign-in of their own, as setup's step and as the account page's dialog.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountCapability

@RunWith(RobolectricTestRunner::class)
class UnreachedScreensTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx(): Context = RuntimeEnvironment.getApplication()

    private val choices = mutableListOf<UnreachedChoice>()
    private val connects = mutableListOf<Pair<String, String>>()
    private var skips = 0
    private var dismissed = 0
    private var busy by mutableStateOf(false)
    private var error by mutableStateOf<String?>(null)

    private val form = SeparateServerForm(
        accountId = "alice@imap",
        uses = listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS),
        set = UnreachedSet.BOTH,
        host = "dav.example.org",
        username = "alice@example.org",
    )

    private fun question(set: UnreachedSet) {
        compose.setContent { SignInUnreachedDialog(set) { choices += it } }
        compose.waitForIdle()
    }

    private fun step() {
        compose.setContent {
            SeparateServerStepScreen(form, busy, error, { u, p -> connects += u to p }, { skips++ })
        }
        compose.waitForIdle()
    }

    @Test
    fun the_question_names_what_the_sign_in_leaves_behind() {
        question(UnreachedSet.CALENDAR)

        compose.onNodeWithText(L10n.signin_unreached_title_calendar(ctx())).assertExists()
        compose.onNodeWithText(L10n.signin_unreached_body(ctx())).assertExists()
    }

    @Test
    fun the_question_titles_both() {
        question(UnreachedSet.BOTH)
        compose.onNodeWithText(L10n.signin_unreached_title_both(ctx())).assertExists()
    }

    @Test
    fun keeping_them_on_the_password_is_one_answer() {
        question(UnreachedSet.BOTH)

        compose.onNodeWithText(L10n.signin_unreached_keep(ctx())).performClick()

        assertEquals(listOf(UnreachedChoice.KEEP), choices)
    }

    @Test
    fun stopping_and_cancelling_are_the_other_two() {
        question(UnreachedSet.CONTACTS)
        compose.onNodeWithText(L10n.signin_unreached_title_contacts(ctx())).assertExists()

        compose.onNodeWithTag("signin-unreached-stop").performClick()
        compose.onNodeWithTag("signin-unreached-cancel").performClick()

        assertEquals(listOf(UnreachedChoice.STOP, UnreachedChoice.CANCEL), choices)
    }

    @Test
    fun the_step_names_the_server_and_starts_from_its_login() {
        step()

        compose.onNodeWithText(L10n.separate_server_title_both(ctx())).assertExists()
        compose.onNodeWithText(L10n.separate_server_body(ctx(), "dav.example.org")).assertExists()
        compose.onNodeWithText("alice@example.org").assertExists()
        compose.onNodeWithTag("separate-server-connect").assertIsNotEnabled()
    }

    @Test
    fun connect_tries_the_login_and_the_password_typed() {
        step()

        compose.onNodeWithText(L10n.setup_field_password(ctx())).performTextInput("secret")
        compose.onNodeWithTag("separate-server-connect").assertIsEnabled().performClick()

        assertEquals(listOf("alice@example.org" to "secret"), connects)
        assertEquals(0, skips)
    }

    @Test
    fun skip_goes_on_without_connecting() {
        step()

        compose.onNodeWithTag("separate-server-skip").performClick()

        assertEquals(1, skips)
        assertEquals(emptyList<Pair<String, String>>(), connects)
    }

    @Test
    fun a_failure_is_said_under_the_fields_and_a_connect_under_way_holds_still() {
        error = L10n.signin_refused(ctx())
        step()

        compose.onNodeWithTag("separate-server-error").assertExists()
        compose.onNodeWithText(L10n.signin_refused(ctx())).assertExists()

        busy = true
        compose.waitForIdle()
        compose.onNodeWithTag("separate-server-connect").assertIsNotEnabled()
        compose.onNodeWithTag("separate-server-skip").assertIsNotEnabled()
    }

    @Test
    fun the_page_dialog_offers_cancel_in_place_of_skip() {
        compose.setContent {
            SeparateServerDialog(form, busy, error, { u, p -> connects += u to p }, { dismissed++ })
        }
        compose.waitForIdle()

        compose.onNodeWithText(L10n.separate_server_title_both(ctx())).assertExists()
        compose.onNodeWithText(L10n.setup_field_password(ctx())).performTextInput("secret")
        compose.onNodeWithTag("separate-server-connect").performClick()
        compose.onNodeWithTag("separate-server-cancel").performClick()

        assertEquals(listOf("alice@example.org" to "secret"), connects)
        assertEquals(1, dismissed)
    }
}
