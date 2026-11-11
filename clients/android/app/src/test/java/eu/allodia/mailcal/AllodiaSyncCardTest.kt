// Settings → Accounts, the section about the person's other devices, drawn from the core's
// `allodia_devices`.
//
// Its rules are all about silence. A device that has not asked yet must look the same as one with
// nothing to report, nothing on screen, because a heading with an empty list under it reads as
// "your other devices have no accounts", which is a claim this app has not earned until a pass has
// run. And a question's answer is a change on its account's page, held and running as the core
// says.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
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
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AllodiaAccountKind
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.AllodiaDevices
import uniffi.mailcal_bindings.AllodiaDevicesQuestion
import uniffi.mailcal_bindings.AllodiaElsewhere
import uniffi.mailcal_bindings.AllodiaSyncFailure
import uniffi.mailcal_bindings.ControlState

private const val HEADING = "From your other devices"
private const val SET_UP = "Set up"
private const val KEEP = "Keep this device’s settings"

@RunWith(RobolectricTestRunner::class)
class AllodiaSyncCardTest {
    @get:Rule val compose = createComposeRule()

    private var setUp = mutableListOf<String>()
    private var changes = mutableListOf<AccountPageIntent>()
    private var signedInAgain = 0

    private fun card(devices: AllodiaDevices?) {
        compose.setContent {
            AllodiaSyncCard(
                devices,
                onSetUp = { setUp += it.email },
                onChange = { changes += it },
                onSignInAgain = { signedInAgain += 1 },
            )
        }
        compose.waitForIdle()
    }

    private fun devices(
        offers: List<AllodiaAccountOffer> = emptyList(),
        questions: List<AllodiaDevicesQuestion> = emptyList(),
        checking: Boolean = false,
        failure: AllodiaSyncFailure? = null,
        nothingFound: Boolean = false,
    ) = AllodiaDevices(
        checking = checking,
        nothingFound = nothingFound,
        offers = offers,
        questions = questions,
        failure = failure,
    )

    private fun question(
        elsewhere: AllodiaElsewhere,
        keep: ControlState = FREE,
        failed: AllodiaSyncFailure? = null,
    ) = AllodiaDevicesQuestion(
        accountId = "acct-1",
        email = "someone@example.com",
        elsewhere = elsewhere,
        keep = keep,
        failed = failed,
    )

    private fun offer(email: String) = AllodiaAccountOffer(
        id = "abc",
        email = email,
        kind = AllodiaAccountKind.IMAP,
        host = "imap.example.com",
        port = 993u,
        security = null,
        smtpHost = null,
        smtpPort = null,
        smtpSecurity = null,
        caldavBaseUrl = null,
        jmapBaseUrl = null,
    )

    @Test
    fun a_device_that_has_not_asked_yet_says_nothing() {
        card(null)
        compose.onNodeWithText(HEADING).assertDoesNotExist()
    }

    @Test
    fun a_pass_that_found_nothing_also_says_nothing() {
        card(devices(nothingFound = true))
        compose.onNodeWithText(HEADING).assertDoesNotExist()
    }

    @Test
    fun a_pass_running_says_it_is_checking() {
        card(devices(checking = true))
        compose.onNodeWithText("Checking your other devices…").assertIsDisplayed()
    }

    /**
     * An offer names the address, because that is what a person recognises, and setting it up
     * hands that address back, so the setup flow opens with the typing already done.
     */
    @Test
    fun an_offer_names_the_address_and_sets_it_up_by_it() {
        card(devices(offers = listOf(offer("someone@example.com"))))
        compose.onNodeWithText(HEADING).assertIsDisplayed()
        compose.onNodeWithText("someone@example.com").assertIsDisplayed()

        compose.onNodeWithText(SET_UP).performClick()

        assertEquals(listOf("someone@example.com"), setUp)
    }

    /**
     * A conflict and a removal are both questions, and keeping this device's settings pauses the
     * ACCOUNT's sharing as a change on its page: the address is what a person reads, the id is
     * what the core changes.
     */
    @Test
    fun keeping_this_devices_settings_pauses_the_account_on_its_page() {
        card(devices(questions = listOf(question(AllodiaElsewhere.CHANGED))))
        compose.onNodeWithText("someone@example.com was changed on another device.")
            .assertIsDisplayed()

        compose.onNodeWithText(KEEP).performClick()

        assertEquals(
            listOf(AccountPageIntent.SetSharing("acct-1", AllodiaAccountSyncMode.PAUSED)),
            changes,
        )
    }

    @Test
    fun an_account_removed_elsewhere_says_so_and_can_be_kept() {
        card(devices(questions = listOf(question(AllodiaElsewhere.REMOVED))))
        compose.onNodeWithText("someone@example.com was removed on another device.")
            .assertIsDisplayed()

        compose.onNodeWithText(KEEP).performClick()

        assertEquals(
            listOf(AccountPageIntent.SetSharing("acct-1", AllodiaAccountSyncMode.PAUSED)),
            changes,
        )
    }

    /** A Keep running is held with its spinner, so it cannot be pressed twice. */
    @Test
    fun a_keep_running_is_held_with_a_spinner() {
        val running = ControlState(enabled = false, running = true)
        card(devices(questions = listOf(question(AllodiaElsewhere.CHANGED, keep = running))))
        compose.onNodeWithTag("allodia-keep-acct-1-running").assertExists()
        compose.onNodeWithTag("allodia-keep-acct-1").assertIsNotEnabled().performClick()
        assertEquals(emptyList<AccountPageIntent>(), changes)
    }

    /** A Keep that did not take is said under its question, and the question stays. */
    @Test
    fun a_keep_that_did_not_take_says_so_under_the_question() {
        val failed = question(AllodiaElsewhere.CHANGED, failed = AllodiaSyncFailure.UNAVAILABLE)
        card(devices(questions = listOf(failed)))
        compose.onNodeWithText(
            "Couldn’t change how this account is synced with your other devices. Nothing changed.",
        ).assertIsDisplayed()
        compose.onNodeWithText(KEEP).assertIsDisplayed()
    }

    /**
     * A pass that could not reach the service says so rather than looking like a pass that found
     * nothing: the two are the same picture otherwise, and only one of them is worth retrying.
     * `Unavailable` says nothing about the sign-in, so it gets one plain sentence and the detail
     * stays in the diagnostic log.
     */
    @Test
    fun a_pass_that_failed_says_so_in_one_plain_line() {
        card(devices(failure = AllodiaSyncFailure.UNAVAILABLE))
        compose.onNodeWithText("Couldn’t check your other devices. It will try again later.")
            .assertIsDisplayed()
    }

    /**
     * A grant that predates a permission is an offer, not an error: they are signed in and one
     * feature is asleep. The remedy is the ordinary sign-in, which asks for the full current set.
     */
    @Test
    fun a_grant_that_predates_the_feature_offers_the_one_thing_that_fixes_it() {
        card(devices(failure = AllodiaSyncFailure.NEEDS_REAUTH))
        compose.onNodeWithText("Sign in again to keep your accounts in step").assertIsDisplayed()

        compose.onNodeWithText("Sign in again").performClick()

        assertEquals(1, signedInAgain)
    }

    /** A grant that is gone is a statement about the account, with the same one way back. */
    @Test
    fun a_revoked_grant_says_they_are_signed_out() {
        card(devices(failure = AllodiaSyncFailure.SIGNED_OUT))
        compose.onNodeWithText("You’re signed out of your Allodia account").assertIsDisplayed()

        compose.onNodeWithText("Sign in").performClick()

        assertEquals(1, signedInAgain)
    }
}
