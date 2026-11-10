// A calendar or address book an account's sign-in does not reach (docs/accounts.md rule 18): which
// title a set of uses gets, whether a sign-in asks first and what each answer tells the core, and
// when setup shows the separate server's step.
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
import uniffi.mailcal_bindings.UnreachedUses
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class UnreachedRulesTest {
    private val both = separateSection(CALENDAR, CONTACTS)

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
    fun the_form_is_the_section_the_core_draws_for_calendar_or_contacts() {
        val form = UnreachedRules.form("alice@imap", both)!!

        assertEquals("alice@imap", form.accountId)
        assertEquals(listOf(CALENDAR, CONTACTS), form.uses)
        assertEquals(UnreachedSet.BOTH, form.set)
        assertEquals(UnreachedSet.CONTACTS, UnreachedRules.form("alice@imap", separateSection(CONTACTS))?.set)
        assertNull("nothing needs one", UnreachedRules.form("alice@imap", null))
        assertNull(UnreachedRules.form("alice@imap", separateSection(MAIL)))
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
    fun setup_holds_the_separate_step_ahead_of_the_link_step() {
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

        assertTrue(flow.accountAdded("alice@imap", false, emptyList(), stepFor, separateForm(both)))

        assertEquals("alice@imap", flow.separate.form?.accountId)
        assertFalse(flow.showsLinks)

        assertTrue(flow.separateDone(stepFor))

        assertNull(flow.separate.form)
        assertTrue(flow.showsLinks)
        assertEquals("alice@imap", flow.links?.account)
    }
}
