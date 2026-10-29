// A core refusal says what went wrong in its own words, never the generated record's `v1=...` form,
// which is what the setup form and an account's page put in front of somebody. A refused sign-in
// is said in the person's language, and the server's reply is kept for the log. A browser returning
// from another sign-in is said as such on every route that completes one, and a calendar or address
// book that did not connect on a server of its own names the use and why.
package eu.allodia.mailcal

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.UseProblem

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

    @Test
    fun a_browser_returning_from_another_sign_in_is_said_as_such_on_every_route() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val stale = MailcalException.StaleSignIn()
        val said = L10n.signin_stale(ctx)
        assertEquals(said, coreErrorSaid(stale, ctx))
        assertEquals(said, separateSignInSaid(stale, ctx))
        assertEquals(said, signInFailedSaid(stale, ctx, L10n.setup_imap_signin_failed(ctx)))
        assertEquals(said, allodiaFailureSaid(stale, ctx))
        assertEquals("the browser returned from another sign-in", coreErrorText(stale))
    }

    @Test
    fun a_completion_says_any_other_failure_as_its_route_always_has() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val failed = MailcalException.Connect("token exchange failed")
        val fallback = L10n.setup_jmap_signin_failed(ctx)
        assertEquals(fallback, signInFailedSaid(failed, ctx, fallback))
        assertEquals(L10n.settings_allodia_failed(ctx, "token exchange failed"), allodiaFailureSaid(failed, ctx))
    }

    @Test
    fun a_use_that_did_not_connect_names_the_use_and_why() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        val cases = listOf(
            AccountCapability.CALENDAR to UseProblem.UNREACHABLE to L10n.separate_server_unreachable_calendar(ctx),
            AccountCapability.CONTACTS to UseProblem.UNREACHABLE to L10n.separate_server_unreachable_contacts(ctx),
            AccountCapability.CALENDAR to UseProblem.NEEDS_OWN_SIGN_IN to
                L10n.settings_account_use_own_sign_in_calendar(ctx),
            AccountCapability.CONTACTS to UseProblem.NEEDS_OWN_SIGN_IN to
                L10n.settings_account_use_own_sign_in_contacts(ctx),
            AccountCapability.CALENDAR to UseProblem.SIGN_IN_REFUSED to L10n.signin_refused(ctx),
            AccountCapability.CONTACTS to UseProblem.SIGN_IN_REFUSED to L10n.signin_refused(ctx),
            AccountCapability.CALENDAR to UseProblem.SERVER_CHANGED to
                L10n.settings_account_use_server_changed_calendar(ctx),
            AccountCapability.CONTACTS to UseProblem.SERVER_CHANGED to
                L10n.settings_account_use_server_changed_contacts(ctx),
            // The core never names mail here; it reads as the calendar.
            AccountCapability.MAIL to UseProblem.UNREACHABLE to L10n.separate_server_unreachable_calendar(ctx),
        )
        for ((use, expected) in cases) {
            val error = MailcalException.UseNotConnected(use.first, use.second)
            assertEquals("$use", expected, coreErrorSaid(error, ctx))
            // The sign-in route says it too, not the sign-in's own failure line.
            assertEquals("$use", expected, separateSignInSaid(error, ctx))
        }
    }

    @Test
    fun a_use_that_did_not_connect_is_logged_in_plain_words() {
        assertEquals(
            "the address book did not connect: its server could not be reached",
            coreErrorText(MailcalException.UseNotConnected(AccountCapability.CONTACTS, UseProblem.UNREACHABLE)),
        )
        assertEquals(
            "the calendar did not connect: its server needs a sign-in of its own",
            coreErrorText(MailcalException.UseNotConnected(AccountCapability.CALENDAR, UseProblem.NEEDS_OWN_SIGN_IN)),
        )
        assertEquals(
            "the calendar did not connect: its server was found on another site and waits to be allowed",
            coreErrorText(MailcalException.UseNotConnected(AccountCapability.CALENDAR, UseProblem.SERVER_CHANGED)),
        )
    }

    @Test
    fun signing_in_again_says_a_stale_or_other_account_sign_in_and_otherwise_the_retry_line() {
        val ctx = ApplicationProvider.getApplicationContext<Context>()
        assertEquals(L10n.signin_stale(ctx), signInAgainSaid(MailcalException.StaleSignIn(), ctx))
        assertEquals(L10n.signin_different_account(ctx), signInAgainSaid(MailcalException.DifferentAccount(), ctx))
        assertEquals(L10n.signin_expired_failed(ctx), signInAgainSaid(MailcalException.Connect("invalid_grant"), ctx))
        assertEquals(L10n.signin_refused(ctx), signInAgainSaid(MailcalException.SignInRefused("401"), ctx))
    }
}
