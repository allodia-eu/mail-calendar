// The steps after a connect (docs/onboarding.md, "After the connect"): which accounts a new one is
// offered to link to, that a suggestion starts picked and survives only until the person changes a
// pick, that "Add another account" comes back to the step it left, the uses a JMAP server offers,
// and the servers found beside it.
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
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class SetupFlowTest {
    private fun snapshot(kind: CoreAccountKind = CoreAccountKind.IMAP, suggested: Boolean = true) =
        AccountsSnapshot(
            listOf(
                accountEntry(
                    kind,
                    MAIL to CapabilityState.ON,
                    id = "alice@imap",
                    candidates = LinkCandidates(
                        listOf(linked("bob@dav"), linked("alice@dav")),
                        emptyList(),
                        emptyList(),
                        if (suggested) listOf("alice@dav") else emptyList(),
                    ),
                ),
            ),
        )

    private fun standards(kind: CoreAccountKind) = kind == CoreAccountKind.IMAP || kind == CoreAccountKind.DAV ||
        kind == CoreAccountKind.JMAP

    private fun stepsOf(snapshot: AccountsSnapshot): (String) -> SetupLinkStep? =
        { SetupLinkStep.of(snapshot, it, ::standards) }

    @Test
    fun a_suggestion_starts_picked_and_continuing_links_it() {
        val step = SetupLinkStep.of(snapshot(), "alice@imap", ::standards)!!
        assertEquals(1, step.pickers.size)
        assertEquals(listOf(1), step.picked)
        assertEquals(listOf(LinkSlot.CALENDAR to "alice@dav"), step.links())
    }

    @Test
    fun nothing_to_link_is_no_step() {
        assertNull(SetupLinkStep.of(snapshot(), "someone-else", ::standards))
        // A provider account is linked from Settings, not offered it at setup.
        assertNull(SetupLinkStep.of(snapshot(CoreAccountKind.GOOGLE), "alice@imap", ::standards))
        val lone = AccountsSnapshot(listOf(accountEntry(CoreAccountKind.IMAP, MAIL to CapabilityState.ON, id = "alice@imap")))
        assertNull(SetupLinkStep.of(lone, "alice@imap", ::standards))
    }

    @Test
    fun a_pick_survives_a_late_suggestion_and_none_links_nothing() {
        var step = SetupLinkStep.of(snapshot(suggested = false), "alice@imap", ::standards)!!
        assertEquals(listOf<Int?>(null), step.picked)
        assertTrue(step.links().isEmpty())

        // Untouched, the step takes the suggestion that arrived.
        step = step.refreshed(SetupLinkStep.of(snapshot(), "alice@imap", ::standards)!!)
        assertEquals(listOf(1), step.picked)

        // Touched, it keeps what the person picked.
        step = step.pick(0, 0)
        val kept = step.refreshed(SetupLinkStep.of(snapshot(), "alice@imap", ::standards)!!)
        assertEquals(listOf(LinkSlot.CALENDAR to "bob@dav"), kept.links())
    }

    @Test
    fun another_account_comes_back_to_the_step_it_was_added_from_and_is_picked_there() {
        val steps = stepsOf(snapshot(suggested = false))
        val flow = SetupFlow()
        assertTrue(flow.accountAdded("alice@imap", false, emptyList(), steps))
        assertTrue(flow.showsLinks)

        flow.addLinked()
        assertFalse(flow.showsLinks)
        assertEquals("alice@imap", flow.returningTo)

        // The account added from there brings the person back to the step they left, with it picked.
        assertTrue(flow.accountAdded("bob@dav", false, emptyList(), steps))
        assertEquals("alice@imap", flow.links?.account)
        assertEquals(listOf(LinkSlot.CALENDAR to "bob@dav"), flow.links?.links())
        assertEquals(listOf("alice@imap", "bob@dav"), flow.finish())
        assertNull(flow.links)
        assertFalse(flow.addedAny)
    }

    @Test
    fun cancelling_another_account_returns_to_the_step_and_a_flow_with_nothing_to_link_ends() {
        val flow = SetupFlow()
        flow.accountAdded("alice@imap", false, emptyList(), stepsOf(snapshot()))
        flow.addLinked()
        assertTrue(flow.cancelled(stepsOf(snapshot())))
        assertEquals("alice@imap", flow.links?.account)

        // Nothing left to return to: the caller ends the flow, and still asks each added name.
        assertFalse(flow.cancelled(stepsOf(snapshot())))
        assertEquals(listOf("alice@imap"), flow.finish())

        assertFalse(SetupFlow().accountAdded("someone-else", false, emptyList(), stepsOf(snapshot())))
    }

    @Test
    fun a_late_suggestion_reaches_an_unchanged_step() {
        val unsuggested = stepsOf(snapshot(suggested = false))
        val flow = SetupFlow()
        flow.accountAdded("alice@imap", false, emptyList(), unsuggested)
        flow.refresh(stepsOf(snapshot())("alice@imap"))
        assertEquals(listOf(1), flow.links?.picked)
    }

    @Test
    fun a_jmap_server_offering_several_uses_asks_which_first() {
        val flow = SetupFlow()
        val choices = listOf(choice(MAIL, true), choice(CALENDAR, true), choice(CONTACTS, true))
        assertTrue(flow.accountAdded("alice@imap", true, choices, stepsOf(snapshot())))
        assertNotNull(flow.uses)
        val step = flow.uses!!
        assertFalse(flow.showsLinks)
        // What was on and is no longer chosen is switched off.
        assertEquals(listOf(CONTACTS to false), step.changes(listOf(MAIL, CALENDAR)))
        assertTrue(flow.usesChosen(stepsOf(snapshot())))
        assertNull(flow.uses)
        assertTrue(flow.showsLinks)
    }

    @Test
    fun a_use_ticked_in_the_uses_step_is_switched_on_before_any_is_switched_off() {
        // A use the account was not set up for starts off, and ticking it is a choice like any other.
        val step = UsesStep.of(
            "alice@jmap",
            "alice@jmap",
            listOf(choice(MAIL, true), choice(CALENDAR, true), choice(CONTACTS, false, found = true)),
        )!!
        assertEquals(listOf(CONTACTS to true, CALENDAR to false), step.changes(listOf(MAIL, CONTACTS)))
        assertTrue(step.changes(listOf(MAIL, CALENDAR)).isEmpty())
    }

    @Test
    fun the_servers_found_beside_a_jmap_server_are_offered_only_for_what_it_lacks() {
        val jmap = accountEntry(
            CoreAccountKind.JMAP,
            MAIL to CapabilityState.ON,
            CALENDAR to CapabilityState.NOT_OFFERED,
            CONTACTS to CapabilityState.ON,
            id = "alice@jmap",
        )
        val found = Beside("alice@jmap", "alice@example.org", "https://dav.example.org/", null)
        val step = SetupLinkStep.of(AccountsSnapshot(listOf(jmap)), "alice@jmap", ::standards, found)!!
        assertEquals(Fills.CALENDAR, step.beside?.second)
        assertEquals("dav.example.org", step.beside?.first?.host)

        // A session with both its own offers nothing beside it, and no step.
        val whole = jmap.copy(uses = jmap.uses.map { it.copy(state = CapabilityState.ON) })
        assertNull(SetupLinkStep.of(AccountsSnapshot(listOf(whole)), "alice@jmap", ::standards, found))
    }

    @Test
    fun the_servers_found_beside_are_kept_for_the_jmap_account_and_used_up() {
        val jmap = accountEntry(
            CoreAccountKind.JMAP,
            MAIL to CapabilityState.ON,
            CALENDAR to CapabilityState.NOT_OFFERED,
            id = "alice@jmap",
        )
        val snapshot = AccountsSnapshot(listOf(jmap))
        val flow = SetupFlow()
        flow.found = FoundBeside("alice@example.org", "https://dav.example.org/", null)
        val stepFor: (String) -> SetupLinkStep? = { flow.besideFor(it)?.let { b -> SetupLinkStep.of(snapshot, it, ::standards, b) } }
        assertTrue(flow.accountAdded("alice@jmap", true, emptyList(), stepFor))
        assertNotNull(flow.links?.beside)

        val offer = flow.addBeside()
        assertEquals("alice@example.org", offer?.email)
        assertEquals("alice@jmap", flow.returningTo)
        assertNull(flow.besideFor("alice@jmap"))
    }
}
