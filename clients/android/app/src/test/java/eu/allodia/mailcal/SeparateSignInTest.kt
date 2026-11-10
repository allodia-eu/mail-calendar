// A separate server's form and its sign-in in the browser (docs/mail-oauth.md rule 13), both the
// core's: when the sign-in is offered, what the form's controls allow while the core holds them,
// what cancels a sign-in still out, and that a form the core draws no section for is over.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability.CALENDAR
import uniffi.mailcal_bindings.AccountCapability.CONTACTS
import uniffi.mailcal_bindings.AccountCapability.MAIL
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountSeparateServer
import uniffi.mailcal_bindings.AccountSeparateSignIn
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class SeparateSignInTest {
    private val both = separateSection(CALENDAR, CONTACTS)
    private val offering = separateSection(CALENDAR, CONTACTS, signIn = AccountSeparateSignIn.PROVIDER)
    private val running = ControlState(enabled = false, running = true)

    private fun opened(section: AccountSeparateServer): SeparateSignIn {
        val state = SeparateSignIn()
        state.open(separateForm(section))
        return state
    }

    @Test
    fun the_sign_in_is_offered_once_the_core_draws_it() {
        val state = opened(both)
        assertFalse(state.controls!!.offersSignIn)
        assertFalse("nothing offered yet", state.signingIn())

        assertTrue(state.redraw(offering))
        assertTrue(state.controls!!.offersSignIn)
        assertTrue(state.signingIn())
    }

    @Test
    fun a_form_the_core_draws_no_section_for_is_over() {
        val state = opened(offering)
        assertFalse(state.redraw(null))
        assertTrue("nothing open has nothing to end", SeparateSignIn().redraw(null))
    }

    @Test
    fun the_sign_in_runs_as_the_core_says_and_a_press_waits_meanwhile() {
        val state = opened(separateSection(CALENDAR, signIn = AccountSeparateSignIn.PROVIDER, signInControl = running))

        assertTrue(state.controls!!.signIn.running)
        assertFalse(state.signingIn())
        assertNull("nothing waits on the browser yet", state.controls!!.waiting)
    }

    @Test
    fun while_the_browser_waits_a_fresh_press_is_taken_and_connect_and_skip_are_held() {
        val state = opened(separateWaiting(CALENDAR))
        val controls = state.controls!!

        assertTrue("a fresh press starts it again", state.signingIn())
        assertEquals(4uL, controls.waiting?.attempt)
        assertFalse(controls.connect.enabled || controls.skip.enabled || controls.editable)
        assertTrue("the way back stays", controls.leavable)
        assertFalse(state.sending())
        assertFalse(state.sending(skip = true))
    }

    @Test
    fun only_a_sign_in_on_the_browser_is_drawn_as_waiting() {
        val waiting = separateWaiting(CALENDAR)
        val completing = waiting.copy(browser = waiting.browser!!.copy(stage = BrowserSignInStage.Completing))

        assertNull(SeparateControls.of(completing).waiting)
    }

    @Test
    fun a_sign_in_out_is_cancelled_by_its_attempt_unless_the_core_holds_its_cancel() {
        val browser = separateWaiting(CALENDAR).browser!!

        assertEquals(AccountPageIntent.CancelSignIn("alice@imap", 4uL), separateCancel(browser))
        assertNull(separateCancel(browser.copy(cancel = HELD)))
        assertNull(separateCancel(null))
    }

    @Test
    fun the_core_holds_connect_and_skip() {
        val free = SeparateControls.of(both)
        assertTrue(free.connect.enabled && free.skip.enabled && free.editable && free.leavable)

        val connecting = SeparateControls.of(separateSection(CALENDAR, connect = running, stopUsing = HELD))
        assertTrue(connecting.connect.running)
        assertFalse(connecting.editable)
        assertFalse(connecting.leavable)

        val skipping = SeparateControls.of(separateSection(CALENDAR, connect = HELD, stopUsing = running))
        assertFalse(skipping.leavable)

        val held = opened(separateSection(CALENDAR, connect = HELD))
        assertFalse(held.sending())
        assertFalse("nothing open", SeparateSignIn().sending())
        assertFalse("nothing open", SeparateSignIn().signingIn())
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
