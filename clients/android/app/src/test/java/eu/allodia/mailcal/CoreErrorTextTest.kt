// A core refusal says what went wrong in its own words, never the generated record's `v1=...` form,
// which is what the setup form and an account's page put in front of somebody. A refused sign-in
// is said in the person's language, and the server's reply is kept for the log.
package eu.allodia.mailcal

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.MailcalException

@RunWith(RobolectricTestRunner::class)
class CoreErrorTextTest {
    @Test
    fun a_refusal_reads_as_its_message() {
        assertEquals("no route to host", coreErrorText(MailcalException.Connect("no route to host")))
        assertEquals("no such account", coreErrorText(MailcalException.Config("no such account")))
        assertEquals("plain", coreErrorText(IllegalStateException("plain")))
    }

    @Test
    fun a_refused_sign_in_is_said_in_words_and_logged_as_the_server_put_it() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val refused = MailcalException.SignInRefused("JMAP HTTP 401: {\"status\":401}")
        assertEquals(L10n.signin_refused(ctx), coreErrorSaid(refused, ctx))
        assertEquals("JMAP HTTP 401: {\"status\":401}", coreErrorText(refused))
        assertEquals("no route to host", coreErrorSaid(MailcalException.Connect("no route to host"), ctx))
    }

    @Test
    fun an_account_already_set_up_is_said_in_words_and_logged_without_its_address() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val twice = MailcalException.AlreadySetUp("alice@example.org@imap.example.org")
        assertEquals(L10n.account_already_set_up(ctx), coreErrorSaid(twice, ctx))
        assertEquals("that account is already set up", coreErrorText(twice))
    }

    @Test
    fun a_sign_in_as_another_account_is_said_in_words() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val other = MailcalException.DifferentAccount()
        assertEquals(L10n.signin_different_account(ctx), coreErrorSaid(other, ctx))
        assertEquals("that sign-in is for a different account", coreErrorText(other))
    }

    @Test
    fun a_separate_servers_sign_in_says_a_failed_exchange_as_the_imap_sign_in_does() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        assertEquals(
            L10n.setup_imap_signin_failed(ctx),
            separateSignInSaid(MailcalException.Connect("token exchange failed"), ctx),
        )
        assertEquals(
            L10n.signin_different_account(ctx),
            separateSignInSaid(MailcalException.DifferentAccount(), ctx),
        )
    }
}
