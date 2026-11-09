// A calendar and contacts account at a Nextcloud server on its page and in the banner
// (docs/nextcloud-sign-in.md rules 7 and 8): which remedy a refused sign-in takes, and the grant
// kept for another try; and the separate-server step's offer.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountSeparateSignIn
import uniffi.mailcal_bindings.AccountProvider

class NextcloudAccountSignInTest {
    @Test
    fun aRefusedNextcloudSignInIsSignedInAgainWhateverItsProviderSays() {
        assertEquals(ExpiredRemedy.SETTINGS, AccountSignInRules.expiredRemedy(AccountProvider.PASSWORD))
        assertEquals(
            ExpiredRemedy.ACCOUNT_SIGN_IN,
            AccountSignInRules.expiredRemedy(AccountProvider.PASSWORD, nextcloud = true),
        )
    }

    @Test
    fun aKeptGrantIsNeverWithdrawnFromUnderTheCoreNorForItsOwnPage() {
        val state = AccountSignInState()
        state.nextcloudGrant = "a" to "granted"
        state.leaveNextcloud(keeping = "a")
        assertEquals("a" to "granted", state.nextcloudGrant)

        assertTrue(state.begin("a", switching = false))
        assertTrue(state.completingNextcloud("a", switching = false))
        state.leaveNextcloud()
        assertEquals("the core is putting it on the account", "a" to "granted", state.nextcloudGrant)

        state.finished()
        state.leaveNextcloud(keeping = "b")
        assertNull(state.nextcloudGrant)
    }

    @Test
    fun aGrantMayNotInterruptAnotherAccountsSignIn() {
        val state = AccountSignInState()
        assertTrue(state.begin("b", switching = false))
        assertFalse(state.completingNextcloud("a", switching = false))
        state.polling()
        assertEquals(SignInStep.BROWSER, state.running?.step)
        assertNull("polling arms no redirect", state.pending)
    }

    @Test
    fun theSeparateStepOffersNextcloudWhereTheCoreSaysSo() {
        val state = SeparateSignIn()
        state.open(separateForm(separateSection(AccountCapability.CALENDAR)))
        state.redraw(separateSection(AccountCapability.CALENDAR, signIn = AccountSeparateSignIn.NEXTCLOUD))
        assertTrue(state.controls!!.offersSignIn && state.controls!!.nextcloud)
        assertTrue(state.offersNextcloud)
        assertTrue(state.beginSignIn() != null)

        state.redraw(separateSection(AccountCapability.CALENDAR, signIn = AccountSeparateSignIn.PROVIDER))
        assertFalse(state.controls!!.nextcloud)
    }
}
