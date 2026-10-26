// A calendar or address book an account's sign-in does not reach (docs/accounts.md rule 18): which
// title a set of uses gets, whether a sign-in asks first and what each answer tells the core, when
// setup shows the separate server's step and the page offers it, and what Skip switches off.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability.CALENDAR
import uniffi.mailcal_bindings.AccountCapability.COLLEAGUES
import uniffi.mailcal_bindings.AccountCapability.CONTACTS
import uniffi.mailcal_bindings.AccountCapability.MAIL
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.SeparateServer
import uniffi.mailcal_bindings.UnreachedUses
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class UnreachedRulesTest {
    private fun mailbox(server: SeparateServer? = null) = accountEntry(
        CoreAccountKind.IMAP,
        MAIL to CapabilityState.ON,
        CALENDAR to CapabilityState.ON,
        CONTACTS to CapabilityState.ON,
        id = "alice@imap",
    ).copy(separateServer = server)

    private val both = SeparateServer(listOf(CALENDAR, CONTACTS), "dav.example.org:8443", "alice")

    @Test
    fun the_title_names_what_is_left_behind() {
        assertEquals(UnreachedSet.BOTH, UnreachedRules.setOf(listOf(CALENDAR, CONTACTS)))
        assertEquals(UnreachedSet.BOTH, UnreachedRules.setOf(listOf(CONTACTS, CALENDAR)))
        assertEquals(UnreachedSet.CALENDAR, UnreachedRules.setOf(listOf(CALENDAR)))
        assertEquals(UnreachedSet.CONTACTS, UnreachedRules.setOf(listOf(CONTACTS)))
        assertNull(UnreachedRules.setOf(emptyList()))
        assertNull(UnreachedRules.setOf(listOf(MAIL, COLLEAGUES)))
    }

    @Test
    fun a_sign_in_asks_first_only_when_it_leaves_something_behind() {
        assertTrue(UnreachedRules.asks(listOf(CALENDAR)))
        assertTrue(UnreachedRules.asks(listOf(CALENDAR, CONTACTS)))
        assertFalse(UnreachedRules.asks(emptyList()))
    }

    @Test
    fun each_answer_tells_the_core_what_was_chosen_and_cancel_tells_it_nothing() {
        assertEquals(UnreachedUses.KEEP_ON_PASSWORD, UnreachedRules.answer(UnreachedChoice.KEEP))
        assertEquals(UnreachedUses.STOP_USING, UnreachedRules.answer(UnreachedChoice.STOP))
        assertNull(UnreachedRules.answer(UnreachedChoice.CANCEL))
    }

    @Test
    fun the_form_is_the_server_the_core_names() {
        val form = UnreachedRules.form(mailbox(both))!!

        assertEquals("alice@imap", form.accountId)
        assertEquals(listOf(CALENDAR, CONTACTS), form.uses)
        assertEquals(UnreachedSet.BOTH, form.set)
        assertEquals("dav.example.org:8443", form.host)
        assertEquals("alice", form.username)
    }

    @Test
    fun a_server_naming_no_login_starts_from_the_address() {
        val form = UnreachedRules.form(mailbox(SeparateServer(listOf(CONTACTS), "dav.example.org", "")))!!

        assertEquals("alice@example.org", form.username)
        assertEquals(UnreachedSet.CONTACTS, form.set)
    }

    @Test
    fun setup_shows_the_step_only_when_a_use_needs_a_sign_in_of_its_own() {
        assertNull(UnreachedRules.form(mailbox()))
        assertNull(UnreachedRules.form(null))
        assertTrue(UnreachedRules.form(mailbox(both)) != null)
    }

    @Test
    fun the_page_offers_the_action_on_exactly_the_uses_named() {
        val calendarOnly = mailbox(SeparateServer(listOf(CALENDAR), "dav.example.org", "alice"))

        assertTrue(UnreachedRules.offersSeparate(calendarOnly, CALENDAR))
        assertFalse(UnreachedRules.offersSeparate(calendarOnly, CONTACTS))
        assertFalse(UnreachedRules.offersSeparate(calendarOnly, MAIL))
        assertFalse(UnreachedRules.offersSeparate(mailbox(), CALENDAR))
    }

    @Test
    fun skip_switches_off_exactly_the_uses_left_behind() {
        assertEquals(listOf(CALENDAR to false), UnreachedRules.skipChanges(listOf(CALENDAR)))
        assertEquals(
            listOf(CALENDAR to false, CONTACTS to false),
            UnreachedRules.skipChanges(listOf(CALENDAR, CONTACTS)),
        )
        assertEquals(emptyList<Any>(), UnreachedRules.skipChanges(listOf(MAIL)))
    }

    @Test
    fun the_refusal_keeps_what_its_sign_in_will_not_reach() {
        val refusal = MailcalException.AlreadySetUp("acct")
        val plain = AccountSignInRules.existingAccount(refusal, "toml") { CoreAccountKind.IMAP }!!
        // A password add's refusal asks nothing.
        assertFalse(UnreachedRules.asks(plain.unreached))

        val signedIn = plain.copy(unreached = listOf(CALENDAR))
        assertTrue(UnreachedRules.asks(signedIn.unreached))
    }

    @Test
    fun a_switch_waits_for_its_answer_and_carries_it_to_the_redirect() {
        val state = AccountSignInState()
        state.begin("acct", switching = true)
        state.ask(UnreachedQuestion("acct", UnreachedSet.CALENDAR, "handle", "https://auth", "redirect"))

        assertEquals(SignInStep.ASKING, state.running?.step)
        assertNull("the browser is not open yet", state.pending)

        val asked = state.decided(UnreachedUses.KEEP_ON_PASSWORD)

        assertEquals("https://auth", asked?.authorizationUrl)
        assertNull(state.question)
        assertEquals(SignInStep.BROWSER, state.running?.step)
        assertEquals(UnreachedUses.KEEP_ON_PASSWORD, state.claim()?.unreached)
    }

    @Test
    fun cancel_ends_the_switch_with_the_browser_never_opened() {
        val state = AccountSignInState()
        state.begin("acct", switching = true)
        state.ask(UnreachedQuestion("acct", UnreachedSet.BOTH, "handle", "https://auth", "redirect"))

        assertNull(state.decided(null))

        assertNull(state.question)
        assertNull(state.pending)
        assertTrue(state.idle)
    }

    @Test
    fun a_sign_in_that_asks_nothing_completes_without_an_answer() {
        val state = AccountSignInState()
        state.begin("acct", switching = true)
        state.opened("handle")

        assertNull(state.claim()?.unreached)
    }

    @Test
    fun setup_holds_the_separate_step_ahead_of_the_link_step() {
        val entry = mailbox(both).copy(
            linkCandidates = LinkCandidates(listOf(linked("bob@dav")), emptyList(), emptyList(), emptyList()),
        )
        val snapshot = AccountsSnapshot(listOf(entry))
        val stepFor: (String) -> SetupLinkStep? = { SetupLinkStep.of(snapshot, it, { true }) }
        val flow = SetupFlow()

        assertTrue(flow.accountAdded("alice@imap", false, emptyList(), stepFor, UnreachedRules.form(entry)))

        assertEquals("alice@imap", flow.separate.form?.accountId)
        assertFalse(flow.showsLinks)

        assertTrue(flow.separateDone(stepFor))

        assertNull(flow.separate.form)
        assertTrue(flow.showsLinks)
        assertEquals("alice@imap", flow.links?.account)
    }

    @Test
    fun a_connect_runs_once_and_a_failure_keeps_the_form() {
        val state = SeparateSignIn()
        assertFalse("nothing to connect", state.start())
        state.open(UnreachedRules.form(mailbox(both))!!)

        assertTrue(state.start())
        assertFalse(state.start())
        state.failed("refused")

        assertEquals("refused", state.error)
        assertFalse(state.busy)
        assertTrue(state.form != null)
    }
}
