// One sign-in at a time for an IMAP or JMAP account, and the redirect it waits for: who claims it,
// and what a return to the app does to it.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AccountSignInStateTest {
    private val state = AccountSignInState()

    @Test
    fun one_flow_runs_at_a_time() {
        assertTrue(state.begin("acct", switching = false))

        assertFalse(state.begin("other", switching = true))
        assertFalse(state.idle)
    }

    @Test
    fun the_redirect_completes_the_sign_in_it_was_opened_for_once() {
        state.begin("acct", switching = true)
        state.opened("handle")
        assertEquals(SignInStep.BROWSER, state.running?.step)

        val claimed = state.claim()

        assertEquals(PendingAccountSignIn("acct", "handle", switching = true), claimed)
        assertEquals(SignInStep.COMPLETING, state.running?.step)
        assertNull(state.claim())
    }

    @Test
    fun coming_back_from_the_browser_stops_waiting_and_keeps_the_redirect_armed() {
        state.begin("acct", switching = false)
        state.opened("handle")

        state.resumed()

        // The browser may still be open behind a password manager.
        assertTrue(state.idle)
        assertEquals("handle", state.claim()?.handle)
    }

    @Test
    fun a_redirect_already_completing_is_left_alone_by_the_return() {
        state.begin("acct", switching = false)
        state.opened("handle")
        state.claim()

        state.resumed()

        assertEquals(SignInStep.COMPLETING, state.running?.step)
    }

    @Test
    fun a_setup_sign_in_takes_the_redirect() {
        state.begin("acct", switching = false)
        state.opened("handle")

        state.yieldRedirect()

        assertNull(state.pending)
        assertTrue(state.idle)
    }

}
