// A calendar and contacts account at a Nextcloud server on its page and in the banner
// (docs/nextcloud-sign-in.md rules 7 and 8): what its Sign-in section offers, which remedy a
// refused sign-in takes, and the grant kept for another try; and the separate-server step's offer.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.AccountSignInOptions
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class NextcloudAccountSignInTest {
    private fun calendar(nextcloud: Boolean) = accountEntry(
        CoreAccountKind.DAV,
        AccountCapability.CALENDAR to CapabilityState.ON,
        signIn = if (nextcloud) SignInMethod.PROVIDER else SignInMethod.PASSWORD,
        nextcloud = nextcloud,
    )

    private val answered = AccountSignInOptions(signIn = false, providerLabel = null, password = false, nextcloud = true)
    private val nothing = AccountSignInOptions(signIn = false, providerLabel = null, password = false)

    @Test
    fun aCalendarAccountHasASectionOnlyAtANextcloudServer() {
        assertFalse(AccountSignInRules.hasSection(calendar(false), null))
        assertFalse(AccountSignInRules.hasSection(calendar(false), nothing))
        assertTrue(AccountSignInRules.hasSection(calendar(false), answered))
        assertTrue("one a Nextcloud sign-in made, before anything is asked", AccountSignInRules.hasSection(calendar(true), null))
        assertTrue(AccountSignInRules.asksOptions(CoreAccountKind.DAV))
    }

    @Test
    fun itSignsInAgainOrSwitchesToNextcloudAndNeverToAPassword() {
        val made = AccountSignInRules.section(calendar(true), answered)
        assertTrue(made.nextcloud && made.signInAgain)
        assertFalse(made.toPassword || made.toProvider || made.toNextcloud || made.offersSwitch)

        val typed = AccountSignInRules.section(calendar(false), answered)
        assertTrue(typed.toNextcloud)
        assertFalse(typed.nextcloud || typed.signInAgain || typed.toPassword)
        assertEquals(SignInMethod.PASSWORD, typed.method)
    }

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
    fun theSeparateStepOffersNextcloudOnlyWhereNoProviderSignInIs() {
        val form = SeparateServerForm("alice", listOf(AccountCapability.CALENDAR), UnreachedSet.CALENDAR, "cloud.example.org", "alice")
        val state = SeparateSignIn()
        state.open(form)
        state.answered(form, offers = false, nextcloud = true)
        assertTrue(state.controls.offersSignIn && state.controls.nextcloud)
        assertTrue(state.beginSignIn() != null)

        val both = SeparateSignIn()
        both.open(form)
        both.answered(form, offers = true, nextcloud = true)
        assertFalse("one sign-in, never two", both.controls.nextcloud)
    }
}
