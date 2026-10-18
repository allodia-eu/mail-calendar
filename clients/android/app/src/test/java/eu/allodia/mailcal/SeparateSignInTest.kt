// A separate server's form and its sign-in at the server's own provider (docs/mail-oauth.md
// rule 13): when the sign-in is offered, what a superseded or retired attempt's answer does, what
// the form's controls allow meanwhile, and that a success goes where Connect's does.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability.CALENDAR
import uniffi.mailcal_bindings.AccountCapability.CONTACTS
import uniffi.mailcal_bindings.AccountCapability.MAIL
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.SeparateServer
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class SeparateSignInTest {
    private fun mailbox(server: SeparateServer) = accountEntry(
        CoreAccountKind.IMAP,
        MAIL to CapabilityState.ON,
        CALENDAR to CapabilityState.ON,
        CONTACTS to CapabilityState.ON,
        id = "alice@imap",
    ).copy(separateServer = server)

    private val both = UnreachedRules.form(mailbox(SeparateServer(listOf(CALENDAR, CONTACTS), "dav.example.org", "alice")))!!
    private val calendar = UnreachedRules.form(mailbox(SeparateServer(listOf(CALENDAR), "dav.example.org", "alice")))!!

    private fun offered(form: SeparateServerForm = both): SeparateSignIn {
        val state = SeparateSignIn()
        state.open(form)
        assertTrue(state.startAsking(form))
        state.answered(form, true)
        return state
    }

    @Test
    fun the_sign_in_is_offered_only_after_the_core_says_so_for_the_forms_uses() {
        val state = SeparateSignIn()
        state.open(both)
        assertFalse(state.offersSignIn)
        assertNull("nothing offered yet", state.beginSignIn())

        assertTrue(state.startAsking(both))
        assertFalse("asked once for each opening", state.startAsking(both))
        assertFalse("not a form on screen", state.startAsking(calendar))

        state.answered(calendar, true)
        assertFalse("an answer for other uses", state.offersSignIn)
        state.answered(both, false)
        assertFalse(state.offersSignIn)
        state.answered(both, true)
        assertTrue(state.offersSignIn)
        assertTrue(state.controls.offersSignIn)
    }

    @Test
    fun an_answer_after_the_form_closed_or_another_opened_is_dropped() {
        val state = SeparateSignIn()
        state.open(both)
        state.startAsking(both)
        state.close()
        state.answered(both, true)
        assertFalse(state.offersSignIn)

        state.open(calendar)
        state.answered(both, true)
        assertFalse(state.offersSignIn)
        assertTrue("the new opening asks again", state.startAsking(calendar))
    }

    @Test
    fun a_superseded_attempt_opens_nothing_and_says_nothing() {
        val state = offered()
        val first = state.beginSignIn()!!
        assertTrue(state.browser.opening(first, "first"))
        state.browser.resumed()

        val second = state.beginSignIn()!!

        assertFalse(state.browser.opening(first, "stale"))
        assertFalse(state.browser.settled(first))
        assertTrue(state.browser.opening(second, "second"))
    }

    @Test
    fun cancel_retires_the_attempt_so_its_start_opens_no_browser() {
        val state = offered()
        val attempt = state.beginSignIn()!!
        assertTrue("the way back stays while it starts", state.controls.leavable)

        state.close()

        assertFalse(state.browser.opening(attempt, "late"))
        assertFalse(state.browser.settled(attempt))
        assertNull(state.browser.redirected())
    }

    @Test
    fun skip_or_connect_supersedes_a_sign_in_waiting_on_the_browser() {
        val state = offered()
        val attempt = state.beginSignIn()!!
        state.browser.opening(attempt, "handle")

        assertTrue(state.start())

        assertNull("its redirect adds nothing", state.browser.redirected())
    }

    @Test
    fun leaving_the_screen_retires_a_waiting_sign_in_but_not_one_completing() {
        val waiting = offered()
        waiting.browser.opening(waiting.beginSignIn()!!, "handle")
        waiting.left()
        assertNull(waiting.browser.pending)

        val completing = offered()
        val attempt = completing.beginSignIn()!!
        completing.browser.opening(attempt, "handle")
        completing.browser.redirected()
        completing.left()
        assertTrue("its account is already being added", completing.browser.settled(attempt))
    }

    @Test
    fun only_the_blocking_calls_hold_the_controls() {
        assertEquals(SeparateControls(), SeparateControls.of(busy = false, offersSignIn = false, step = null))

        val connecting = SeparateControls.of(busy = true, offersSignIn = true, step = null)
        assertTrue(connecting.connecting)
        assertFalse(connecting.editable)
        assertFalse(connecting.leavable)

        val starting = SeparateControls.of(busy = false, offersSignIn = true, step = SeparateSignInStep.STARTING)
        assertTrue(starting.signingIn)
        assertFalse(starting.editable)
        assertTrue(starting.leavable)

        val browser = SeparateControls.of(busy = false, offersSignIn = true, step = SeparateSignInStep.BROWSER)
        assertTrue(browser.signingIn)
        assertTrue("a fresh press supersedes it", browser.editable)
        assertTrue(browser.leavable)

        val completing = SeparateControls.of(busy = false, offersSignIn = true, step = SeparateSignInStep.COMPLETING)
        assertFalse(completing.editable)
        assertFalse(completing.leavable)
    }

    @Test
    fun a_connect_or_skip_waits_while_a_sign_in_completes() {
        val state = offered()
        val attempt = state.beginSignIn()!!
        state.browser.opening(attempt, "handle")
        state.browser.redirected()

        assertFalse(state.start())
        assertNull(state.beginSignIn())
    }

    @Test
    fun a_failed_sign_in_is_said_and_keeps_the_form() {
        val state = offered()
        val attempt = state.beginSignIn()!!

        assertTrue(state.browser.settled(attempt))
        state.failed("refused")

        assertEquals("refused", state.error)
        assertNotNull(state.form)
        assertTrue(state.offersSignIn)
        assertNotNull(state.beginSignIn())
        assertNull("a fresh press clears it", state.error)
    }

    @Test
    fun a_sign_in_that_completes_goes_on_to_the_link_step_as_connect_does() {
        val entry = mailbox(SeparateServer(listOf(CALENDAR, CONTACTS), "dav.example.org", "alice")).copy(
            linkCandidates = LinkCandidates(listOf(linked("bob@dav")), emptyList(), emptyList(), emptyList()),
        )
        val snapshot = AccountsSnapshot(listOf(entry))
        val stepFor: (String) -> SetupLinkStep? = { SetupLinkStep.of(snapshot, it, { true }) }
        val flow = SetupFlow()
        flow.accountAdded("alice@imap", false, emptyList(), stepFor, UnreachedRules.form(entry))
        val form = flow.separate.form!!
        flow.separate.startAsking(form)
        flow.separate.answered(form, true)

        val attempt = flow.separate.beginSignIn()!!
        flow.separate.browser.opening(attempt, "handle")
        flow.separate.browser.redirected()
        assertTrue(flow.separate.browser.settled(attempt))
        // MainActivity.separateLinked, which Connect's success runs too.
        assertTrue(flow.separateDone(stepFor))

        assertNull(flow.separate.form)
        assertTrue(flow.showsLinks)
        assertEquals("alice@imap", flow.links?.account)
    }
}
