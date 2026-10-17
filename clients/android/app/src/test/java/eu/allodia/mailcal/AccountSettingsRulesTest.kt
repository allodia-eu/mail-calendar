// What Settings → Accounts says about an account, from a snapshot entry (docs/accounts.md rules
// 11 to 13). The same rules the Apple, Linux and Windows suites pin.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountLinksView
import uniffi.mailcal_bindings.CapabilityChange
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.UseProblem
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class AccountSettingsRulesTest {
    private fun candidates(
        calendar: List<String> = emptyList(),
        suggested: List<String> = emptyList(),
    ) = LinkCandidates(calendar.map(::linked), emptyList(), emptyList(), suggested)

    @Test
    fun the_last_use_cannot_be_switched_off() {
        val entry = mailboxOnly()
        assertEquals(
            UseSwitch(active = true, asks = false, enabled = false, note = UseNote.LAST),
            AccountSettingsRules.switch(entry, AccountCapability.MAIL),
        )
        assertEquals(
            UseSwitch(active = false, asks = false, enabled = true, note = UseNote.NONE),
            AccountSettingsRules.switch(entry, AccountCapability.CALENDAR),
        )
    }

    @Test
    fun colleagues_wait_for_contacts() {
        fun microsoft(contacts: CapabilityState) = accountEntry(
            CoreAccountKind.MICROSOFT,
            AccountCapability.MAIL to CapabilityState.ON,
            AccountCapability.CALENDAR to CapabilityState.ON,
            AccountCapability.CONTACTS to contacts,
            AccountCapability.COLLEAGUES to CapabilityState.OFF,
        )
        val colleagues = AccountSettingsRules.switch(
            microsoft(CapabilityState.OFF),
            AccountCapability.COLLEAGUES,
        )
        assertFalse(colleagues.enabled)
        assertEquals(UseNote.NEEDS_CONTACTS, colleagues.note)
        assertTrue(
            AccountSettingsRules.switch(microsoft(CapabilityState.ON), AccountCapability.COLLEAGUES)
                .enabled,
        )
    }

    @Test
    fun a_use_waiting_on_permission_is_drawn_off_and_switching_it_on_asks() {
        val entry = accountEntry(
            CoreAccountKind.GOOGLE,
            AccountCapability.MAIL to CapabilityState.ON,
            AccountCapability.CALENDAR to CapabilityState.NEEDS_PERMISSION,
            AccountCapability.CONTACTS to CapabilityState.OFF,
            AccountCapability.COLLEAGUES to CapabilityState.OFF,
        )
        val calendar = AccountSettingsRules.switch(entry, AccountCapability.CALENDAR)
        assertFalse(calendar.active)
        assertTrue(calendar.asks && calendar.enabled)
        assertEquals(UseNote.WITHHELD, calendar.note)
        assertTrue(AccountSettingsRules.needsPermission(entry))
        assertEquals(listOf(AccountCapability.CALENDAR), AccountSettingsRules.withheld(entry))
        // Listed as a use: it is chosen, only not granted.
        assertTrue(AccountCapability.CALENDAR in AccountSettingsRules.inUse(entry))
    }

    @Test
    fun an_only_use_waiting_on_permission_can_still_be_asked_for() {
        val entry = accountEntry(
            CoreAccountKind.GOOGLE,
            AccountCapability.MAIL to CapabilityState.OFF,
            AccountCapability.CALENDAR to CapabilityState.NEEDS_PERMISSION,
            AccountCapability.CONTACTS to CapabilityState.OFF,
        )
        assertEquals(
            UseSwitch(active = false, asks = true, enabled = true, note = UseNote.WITHHELD),
            AccountSettingsRules.switch(entry, AccountCapability.CALENDAR),
        )
    }

    @Test
    fun a_use_the_server_does_not_offer_cannot_be_switched_and_holds_nothing_on() {
        val entry = accountEntry(
            CoreAccountKind.JMAP,
            AccountCapability.MAIL to CapabilityState.ON,
            AccountCapability.CALENDAR to CapabilityState.NOT_OFFERED,
            AccountCapability.CONTACTS to CapabilityState.OFF,
        )
        assertEquals(
            UseSwitch(active = false, asks = false, enabled = false, note = UseNote.NOT_OFFERED),
            AccountSettingsRules.switch(entry, AccountCapability.CALENDAR),
        )
        // Mail is the one use that works, so it is the last.
        assertEquals(UseNote.LAST, AccountSettingsRules.switch(entry, AccountCapability.MAIL).note)
        assertEquals(listOf(AccountCapability.MAIL), AccountSettingsRules.inUse(entry))
    }

    @Test
    fun signing_in_again_is_offered_where_the_provider_signs_in() {
        assertTrue(AccountSettingsRules.signsInAtProvider(CoreAccountKind.MICROSOFT))
        assertTrue(AccountSettingsRules.signsInAtProvider(CoreAccountKind.GOOGLE))
        listOf(CoreAccountKind.IMAP, CoreAccountKind.DAV, CoreAccountKind.JMAP).forEach {
            assertFalse(AccountSettingsRules.signsInAtProvider(it))
        }
    }

    @Test
    fun the_row_names_the_uses_in_order_and_the_links() {
        val entry = mailboxOnly(links = AccountLinksView(linked("alice@cloud.example"), null, null))
        assertEquals(listOf(AccountCapability.MAIL), AccountSettingsRules.inUse(entry))
        assertEquals(
            listOf(AccountCapability.CALENDAR to "alice@cloud.example"),
            AccountSettingsRules.linked(entry),
        )
    }

    @Test
    fun a_picker_is_offered_only_for_a_slot_the_account_can_hold() {
        assertTrue(AccountSettingsRules.linkPickers(mailboxOnly()).isEmpty())

        val entry = mailboxOnly(
            candidates = candidates(calendar = listOf("a@cloud.example", "b@cloud.example")),
            links = AccountLinksView(linked("b@cloud.example"), null, null),
        )
        val picker = AccountSettingsRules.linkPickers(entry).single()
        assertEquals(LinkSlot.CALENDAR, picker.slot)
        assertEquals(2, picker.options.size)
        assertEquals(1, picker.selected)
    }

    @Test
    fun a_linked_account_missing_from_the_candidates_is_still_shown() {
        val entry = mailboxOnly(links = AccountLinksView(null, linked("book@cloud.example"), null))
        val picker = AccountSettingsRules.linkPickers(entry).single()
        assertEquals(LinkSlot.CONTACTS, picker.slot)
        assertEquals(listOf(linked("book@cloud.example")), picker.options)
        assertEquals(0, picker.selected)
    }

    @Test
    fun a_link_whose_address_has_changed_is_not_listed_twice() {
        val stored = uniffi.mailcal_bindings.LinkedAccount("a@cloud.example", "old@cloud.example")
        val entry = mailboxOnly(
            candidates = candidates(calendar = listOf("a@cloud.example")),
            links = AccountLinksView(stored, null, null),
        )
        val picker = AccountSettingsRules.linkPickers(entry).single()
        assertEquals(1, picker.options.size)
        assertEquals(0, picker.selected)
    }

    @Test
    fun a_suggestion_is_offered_only_where_nothing_is_linked() {
        val calendars = listOf("a@cloud.example", "b@cloud.example")
        val open = mailboxOnly(candidates = candidates(calendars, suggested = listOf("b@cloud.example")))
        assertEquals(1, AccountSettingsRules.linkPickers(open).single().suggested)

        // A link already made is not second-guessed.
        val made = mailboxOnly(
            candidates = candidates(calendars, suggested = listOf("b@cloud.example")),
            links = AccountLinksView(linked("a@cloud.example"), null, null),
        )
        assertNull(AccountSettingsRules.linkPickers(made).single().suggested)
    }

    @Test
    fun removing_names_the_accounts_that_lose_their_link() {
        assertNull(AccountSettingsRules.unlinked(mailboxOnly()))
        val entry = mailboxOnly(linkedFrom = listOf(linked("a@example.org"), linked("b@example.org")))
        assertEquals("a@example.org, b@example.org", AccountSettingsRules.unlinked(entry))
    }

    @Test
    fun switching_on_a_use_the_provider_withholds_asks_for_it() {
        assertEquals(
            UseChangeOutcome.ASK_PROVIDER,
            AccountSettingsRules.outcome(CapabilityChange.NEEDS_CONSENT),
        )
        assertEquals(
            UseChangeOutcome.NEEDS_ENDPOINT,
            AccountSettingsRules.outcome(CapabilityChange.NEEDS_ENDPOINT),
        )
        assertEquals(UseChangeOutcome.APPLIED, AccountSettingsRules.outcome(CapabilityChange.APPLIED))
    }

    @Test
    fun a_use_that_did_not_connect_says_why_beneath_its_switch() {
        val problems = mapOf(
            AccountCapability.CALENDAR to UseProblem.NEEDS_OWN_SIGN_IN,
            AccountCapability.CONTACTS to UseProblem.UNREACHABLE,
        )
        val base = accountEntry(
            CoreAccountKind.IMAP,
            AccountCapability.MAIL to CapabilityState.ON,
            AccountCapability.CALENDAR to CapabilityState.ON,
            AccountCapability.CONTACTS to CapabilityState.ON,
        )
        val entry = base.copy(uses = base.uses.map { it.copy(problem = problems[it.capability]) })
        val calendar = AccountSettingsRules.switch(entry, AccountCapability.CALENDAR)
        assertEquals(UseNote.OWN_SIGN_IN_CALENDAR, calendar.note)
        assertTrue("the account is still used for it", calendar.active)
        assertEquals(
            UseNote.UNREACHABLE_CONTACTS,
            AccountSettingsRules.switch(entry, AccountCapability.CONTACTS).note,
        )
        assertEquals(UseNote.NONE, AccountSettingsRules.switch(entry, AccountCapability.MAIL).note)
    }
}
