// Settings → Accounts, the section about the person's other devices.
//
// Its rules are all about silence. A device that has not asked yet must look the same as one with
// nothing to report, nothing on screen, because a heading with an empty list under it reads as
// "your other devices have no accounts", which is a claim this app has not earned until a pass has
// run. And an answered question has to leave: a row still offering "keep this device's settings"
// after it was pressed reads as the press having done nothing.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AllodiaAccountChange
import uniffi.mailcal_bindings.AllodiaGrantHealth
import uniffi.mailcal_bindings.AllodiaAccountKind
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaSyncReport

private const val HEADING = "From your other devices"
private const val SET_UP = "Set up"
private const val KEEP = "Keep this device’s settings"

@RunWith(RobolectricTestRunner::class)
class AllodiaSyncCardTest {
    @get:Rule val compose = createComposeRule()

    private var setUp = mutableListOf<String>()
    private var kept = mutableListOf<String>()
    private var signedInAgain = 0

    private fun card(state: AllodiaSyncState) {
        compose.setContent {
            AllodiaSyncCard(
                state,
                onSetUp = { setUp += it.email },
                onKeepLocal = { kept += it },
                onSignInAgain = { signedInAgain += 1 },
            )
        }
        compose.waitForIdle()
    }

    private fun report(
        offers: List<AllodiaAccountOffer> = emptyList(),
        changed: List<AllodiaAccountChange> = emptyList(),
        removed: List<AllodiaAccountChange> = emptyList(),
    ) = AllodiaSyncReport(
        offers = offers,
        changedElsewhere = changed,
        removedElsewhere = removed,
        sent = 0u,
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
        card(AllodiaSyncState())
        compose.onNodeWithText(HEADING).assertDoesNotExist()
    }

    @Test
    fun a_pass_that_found_nothing_also_says_nothing() {
        card(AllodiaSyncState(report = report()))
        compose.onNodeWithText(HEADING).assertDoesNotExist()
    }

    /**
     * An offer names the address, because that is what a person recognises, and setting it up
     * hands that address back, so the setup flow opens with the typing already done.
     */
    @Test
    fun an_offer_names_the_address_and_sets_it_up_by_it() {
        card(AllodiaSyncState(report = report(offers = listOf(offer("someone@example.com")))))
        compose.onNodeWithText(HEADING).assertIsDisplayed()
        compose.onNodeWithText("someone@example.com").assertIsDisplayed()

        compose.onNodeWithText(SET_UP).performClick()

        assertEquals(listOf("someone@example.com"), setUp)
    }

    /**
     * A conflict and a removal are both questions, and the one answer this device can act on names
     * the ACCOUNT, not the address: the address is what a person reads, the id is what the core
     * detaches.
     */
    @Test
    fun keeping_this_devices_settings_names_the_account_and_not_the_address() {
        card(
            AllodiaSyncState(
                report = report(
                    changed = listOf(
                        AllodiaAccountChange(
                            accountId = "someone@example.com",
                            email = "someone@example.com",
                            alsoChangedHere = true,
                        ),
                    ),
                ),
            ),
        )
        compose.onNodeWithText("someone@example.com was changed on another device.")
            .assertIsDisplayed()

        compose.onNodeWithText(KEEP).performClick()

        assertEquals(listOf("someone@example.com"), kept)
    }

    @Test
    fun an_account_removed_elsewhere_says_so_and_can_be_kept() {
        card(
            AllodiaSyncState(
                report = report(
                    removed = listOf(
                        AllodiaAccountChange(
                            accountId = "someone@example.com",
                            email = "someone@example.com",
                            alsoChangedHere = false,
                        ),
                    ),
                ),
            ),
        )
        compose.onNodeWithText("someone@example.com was removed on another device.")
            .assertIsDisplayed()

        compose.onNodeWithText(KEEP).performClick()

        assertEquals(listOf("someone@example.com"), kept)
    }

    /**
     * A pass that could not reach the service says so rather than looking like a pass that found
     * nothing, the two are the same picture otherwise, and only one of them is worth retrying.
     *
     * What it must NOT say is the failure's own text. `Ok` means the failure said nothing about
     * the sign-in, so it gets one plain sentence and the detail stays in the diagnostic log; the
     * text is what put `invalid_scope, unable to issue scope mailcal:accounts:read` on screen.
     */
    @Test
    fun a_pass_that_failed_says_so_without_quoting_itself() {
        card(
            AllodiaSyncState(
                failure = "oauth endpoint error: invalid_scope",
                health = AllodiaGrantHealth.OK,
            ),
        )
        compose.onNodeWithText("Couldn’t check your other devices. It will try again later.")
            .assertIsDisplayed()
        compose.onNodeWithText("invalid_scope", substring = true).assertDoesNotExist()
    }

    /**
     * A grant that predates a permission is an offer, not an error: they are signed in and one
     * feature is asleep. The remedy is the ordinary sign-in, which asks for the full current set.
     */
    @Test
    fun a_grant_that_predates_the_feature_offers_the_one_thing_that_fixes_it() {
        card(
            AllodiaSyncState(
                failure = "oauth endpoint error: invalid_scope",
                health = AllodiaGrantHealth.NEEDS_REAUTH,
            ),
        )
        compose.onNodeWithText("Sign in again to keep your accounts in step").assertIsDisplayed()

        compose.onNodeWithText("Sign in again").performClick()

        assertEquals(1, signedInAgain)
    }

    /** A grant that is gone is a statement about the account, with the same one way back. */
    @Test
    fun a_revoked_grant_says_they_are_signed_out() {
        card(
            AllodiaSyncState(
                failure = "oauth endpoint error: invalid_grant",
                health = AllodiaGrantHealth.SIGNED_OUT,
            ),
        )
        compose.onNodeWithText("You’re signed out of your Allodia account").assertIsDisplayed()

        compose.onNodeWithText("Sign in").performClick()

        assertEquals(1, signedInAgain)
    }
}
