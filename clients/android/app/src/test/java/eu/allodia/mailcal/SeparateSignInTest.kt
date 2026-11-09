// A separate server's form and its sign-in at the server's own provider (docs/mail-oauth.md
// rule 13): when the sign-in is offered, what a superseded or retired attempt's answer does, what
// the form's controls allow meanwhile, and that a form the core draws no section for is over.
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
import uniffi.mailcal_bindings.AccountSeparateSignIn
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class SeparateSignInTest {
    private val both = separateSection(CALENDAR, CONTACTS)
    private val offering = separateSection(CALENDAR, CONTACTS, signIn = AccountSeparateSignIn.PROVIDER)

    private fun offered(): SeparateSignIn {
        val state = SeparateSignIn()
        state.open(separateForm(offering))
        return state
    }

    @Test
    fun the_sign_in_is_offered_once_the_core_draws_it() {
        val state = SeparateSignIn()
        state.open(separateForm(both))
        assertFalse(state.controls!!.offersSignIn)
        assertNull("nothing offered yet", state.beginSignIn())

        assertTrue(state.redraw(offering))
        assertTrue(state.controls!!.offersSignIn)
        assertNotNull(state.beginSignIn())
    }

    @Test
    fun a_form_the_core_draws_no_section_for_is_over() {
        val state = offered()
        assertFalse(state.redraw(null))
        assertTrue("nothing open has nothing to end", SeparateSignIn().redraw(null))
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
        assertTrue("the way back stays while it starts", state.controls!!.leavable)

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

        assertTrue(state.sending())

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
    fun the_core_holds_connect_and_skip_and_the_blocking_calls_hold_the_rest() {
        val free = SeparateControls.of(both, null)
        assertTrue(free.connect.enabled && free.skip.enabled && free.editable && free.leavable)

        val connecting = SeparateControls.of(
            separateSection(CALENDAR, connect = ControlState(false, true), stopUsing = ControlState(false, false)),
            null,
        )
        assertTrue(connecting.connect.running)
        assertFalse(connecting.editable)
        assertFalse(connecting.leavable)

        val starting = SeparateControls.of(offering, SeparateSignInStep.STARTING)
        assertTrue(starting.signingIn)
        assertFalse(starting.editable)
        assertFalse(starting.connect.enabled)
        assertTrue(starting.leavable)

        val browser = SeparateControls.of(offering, SeparateSignInStep.BROWSER)
        assertTrue(browser.signingIn)
        assertTrue("a fresh press supersedes it", browser.editable)
        assertTrue(browser.leavable)

        val completing = SeparateControls.of(offering, SeparateSignInStep.COMPLETING)
        assertFalse(completing.editable)
        assertFalse(completing.leavable)
        assertFalse(completing.skip.enabled)
    }

    @Test
    fun a_connect_or_skip_waits_while_a_sign_in_completes_or_the_core_holds_it() {
        val state = offered()
        val attempt = state.beginSignIn()!!
        state.browser.opening(attempt, "handle")
        state.browser.redirected()

        assertFalse(state.sending())
        assertFalse(state.sending(skip = true))
        assertNull(state.beginSignIn())

        val held = SeparateSignIn()
        held.open(separateForm(separateSection(CALENDAR, connect = ControlState(false, false))))
        assertFalse(held.sending())
        assertFalse("nothing open", SeparateSignIn().sending())
    }

    @Test
    fun a_failed_sign_in_is_said_and_keeps_the_form() {
        val state = offered()
        val attempt = state.beginSignIn()!!

        assertTrue(state.browser.settled(attempt))
        state.failed("refused")

        assertEquals("refused", state.error)
        assertNotNull(state.form)
        assertNotNull(state.beginSignIn())
        assertNull("a fresh press clears it", state.error)
    }

    @Test
    fun a_form_that_is_over_goes_on_to_the_link_step() {
        val entry = accountEntry(
            CoreAccountKind.IMAP,
            MAIL to CapabilityState.ON,
            CALENDAR to CapabilityState.ON,
            CONTACTS to CapabilityState.ON,
            id = "alice@imap",
            candidates = LinkCandidates(listOf(linked("bob@dav")), emptyList(), emptyList(), emptyList()),
        )
        val snapshot = AccountsSnapshot(listOf(entry))
        val stepFor: (String) -> SetupLinkStep? = { SetupLinkStep.of(snapshot, it, { true }, ::setupPickers) }
        val flow = SetupFlow()
        flow.accountAdded("alice@imap", false, emptyList(), stepFor, separateForm(offering))

        assertFalse(flow.separate.redraw(null))
        // MainActivity.separateLinked, which a form the core no longer draws runs.
        assertTrue(flow.separateDone(stepFor))

        assertNull(flow.separate.form)
        assertTrue(flow.showsLinks)
        assertEquals("alice@imap", flow.links?.account)
    }
}
