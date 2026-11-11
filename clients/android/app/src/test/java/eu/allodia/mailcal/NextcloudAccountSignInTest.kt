// Nextcloud on the separate server's step (docs/nextcloud-sign-in.md rule 7): offered where the
// core says so. A calendar and contacts account's own Nextcloud sign-in is the core's
// (BrowserSignInDriverTest, SettingsAccountSignInTest).
package eu.allodia.mailcal

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountSeparateSignIn

class NextcloudAccountSignInTest {
    @Test
    fun theSeparateStepOffersNextcloudWhereTheCoreSaysSo() {
        val state = SeparateSignIn()
        state.open(separateForm(separateSection(AccountCapability.CALENDAR)))
        state.redraw(separateSection(AccountCapability.CALENDAR, signIn = AccountSeparateSignIn.NEXTCLOUD))
        assertTrue(state.controls!!.offersSignIn && state.controls!!.nextcloud)
        assertTrue(state.signingIn())

        state.redraw(separateSection(AccountCapability.CALENDAR, signIn = AccountSeparateSignIn.PROVIDER))
        assertFalse(state.controls!!.nextcloud)
    }
}
