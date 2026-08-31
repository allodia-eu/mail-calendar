// The setup form's JMAP tab. Two things here are load-bearing and invisible once they're wrong:
// which fields make Connect tappable (JMAP needs an email and ONE secret, a password and an API
// token are the same thing to the core now that the engine negotiates the scheme, and there is no
// second box to leave empty, and never needs a server), and what the form hands the core: a blank
// server must arrive as null, since an empty-string URL would be taken as a real server instead of
// "discover it from my email domain".
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.JmapSetup

private const val TAB_IMAP = "IMAP"
private const val TAB_JMAP = "JMAP"
private const val TAB_MICROSOFT = "Microsoft"
private const val FIELD_EMAIL = "Email"
private const val FIELD_SECRET = "Password or API token"
private const val FIELD_SERVER = "JMAP server (optional, e.g. mail.example.com)"
private const val FIELD_IMAP_HOST = "Mail server"
private const val CONNECT = "Connect"

@RunWith(RobolectricTestRunner::class)
class AccountSetupJmapTest {
    @get:Rule val compose = createComposeRule()

    private val jmapSetups = mutableListOf<JmapSetup>()
    private val imapSetups = mutableListOf<AccountSetup>()

    /** Renders the form, recording whichever setup Connect hands back to the core. */
    private fun form() {
        compose.setContent {
            AccountSetupScreen(
                onConnect = { imapSetups += it; null },
                onConnectJmap = { jmapSetups += it; null },
            )
        }
    }

    /** Renders the form and switches to the JMAP tab. */
    private fun jmapTab() {
        form()
        compose.onNodeWithText(TAB_JMAP).performClick()
        compose.waitForIdle()
    }

    private fun type(field: String, text: String) {
        compose.onNodeWithText(field).performTextInput(text)
        compose.waitForIdle()
    }

    @Test
    fun the_form_offers_jmap_alongside_imap_and_microsoft() {
        form()

        // Exact matches: the first tab reads "IMAP", not the old "IMAP / password", the three
        // labels only fit on one line short.
        compose.onNodeWithText(TAB_IMAP).assertIsDisplayed()
        compose.onNodeWithText(TAB_JMAP).assertIsDisplayed()
        compose.onNodeWithText(TAB_MICROSOFT).assertIsDisplayed()
    }

    @Test
    fun the_jmap_tab_swaps_in_the_jmap_fields() {
        jmapTab()

        compose.onNodeWithText(FIELD_EMAIL).assertIsDisplayed()
        compose.onNodeWithText(FIELD_SECRET).assertIsDisplayed()
        compose.onNodeWithText(FIELD_SERVER).performScrollTo().assertIsDisplayed()
        // JMAP discovers the server from the email domain, so the IMAP host field is gone.
        compose.onNodeWithText(FIELD_IMAP_HOST).assertDoesNotExist()
        // The separate "API token" box is gone, one secret field, labelled for both cases. A
        // second box is what sent the original Fastmail bug report: the user picked the wrong one.
        compose.onNodeWithText("API token (if your server uses one)").assertDoesNotExist()
    }

    @Test
    fun connect_stays_disabled_until_an_email_and_the_secret_are_entered() {
        jmapTab()

        compose.onNodeWithText(CONNECT).assertIsNotEnabled()

        type(FIELD_EMAIL, "alice@test.local")
        // An address alone can't authenticate.
        compose.onNodeWithText(CONNECT).assertIsNotEnabled()

        type(FIELD_SECRET, "secret")
        compose.onNodeWithText(CONNECT).assertIsEnabled()
    }

    @Test
    fun an_api_token_goes_in_the_same_field_as_a_password() {
        // The collapse's user-visible promise: a Fastmail API token is typed into the one secret
        // box and reaches the core as `password`, so the engine can present it under whichever
        // scheme the server challenges for. Nothing asks the user which kind of secret they hold.
        jmapTab()

        type(FIELD_EMAIL, "alice@test.local")
        type(FIELD_SECRET, "fmapi-token")
        compose.onNodeWithText(CONNECT).performScrollTo().performClick()
        compose.waitForIdle()

        assertEquals("fmapi-token", jmapSetups.single().password)
    }

    @Test
    fun connecting_sends_the_typed_fields_and_nulls_the_blank_ones() {
        jmapTab()

        type(FIELD_EMAIL, "alice@test.local")
        type(FIELD_SECRET, "secret")
        compose.onNodeWithText(CONNECT).performScrollTo().performClick()
        compose.waitForIdle()

        assertEquals(
            JmapSetup(
                email = "alice@test.local",
                serverUrl = null,
                password = "secret",
            ),
            jmapSetups.single(),
        )
        // The JMAP tab must never fall through to the IMAP connect path.
        assertTrue("IMAP path was not taken", imapSetups.isEmpty())
    }

    @Test
    fun a_typed_server_is_passed_through() {
        jmapTab()

        type(FIELD_EMAIL, "alice@test.local")
        type(FIELD_SECRET, "tok_123")
        type(FIELD_SERVER, "http://127.0.0.1:18080")
        compose.onNodeWithText(CONNECT).performScrollTo().performClick()
        compose.waitForIdle()

        assertEquals(
            JmapSetup(
                email = "alice@test.local",
                serverUrl = "http://127.0.0.1:18080",
                password = "tok_123",
            ),
            jmapSetups.single(),
        )
    }
}
