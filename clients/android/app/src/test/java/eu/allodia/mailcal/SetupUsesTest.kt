// What a setup card's toggles make of an account (docs/account-autodetect.md rule 8,
// docs/accounts.md rules 2 and 10): which uses it is set up with, and which servers it stores.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountCapability.CALENDAR
import uniffi.mailcal_bindings.AccountCapability.COLLEAGUES
import uniffi.mailcal_bindings.AccountCapability.CONTACTS
import uniffi.mailcal_bindings.AccountCapability.MAIL
import uniffi.mailcal_bindings.AccountCapability.SHARED_MAILBOXES

private const val CALDAV = "https://dav.example.org/caldav"
private const val CARDDAV = "https://dav.example.org/carddav"

class SetupUsesTest {
    private val on = PickedUse(true)
    private val off = PickedUse(false)

    private fun offer(caldav: String, carddav: String) = UseOffer(
        listOf(
            choice(MAIL, true),
            choice(CALENDAR, caldav.isNotEmpty()),
            choice(CONTACTS, caldav.isNotEmpty() || carddav.isNotEmpty()),
        ),
        caldav,
        carddav,
    )

    private fun provider() = UseOffer(listOf(MAIL, CALENDAR, CONTACTS, COLLEAGUES).map { choice(it, true) })

    @Test
    fun everything_found_and_kept_stores_both_endpoints_and_every_use() {
        val chosen = SetupUses.chosen(offer(CALDAV, CARDDAV), true, on, on, null)
        assertEquals(listOf(MAIL, CALENDAR, CONTACTS), chosen.uses)
        assertEquals(CALDAV, chosen.caldavUrl)
        assertEquals(CARDDAV, chosen.carddavUrl)
    }

    @Test
    fun contacts_found_through_the_calendar_keep_its_server_with_the_calendar_off() {
        val kept = SetupUses.chosen(offer(CALDAV, ""), true, off, on, null)
        assertEquals(listOf(MAIL, CONTACTS), kept.uses)
        assertEquals(CALDAV, kept.caldavUrl)
        assertEquals("", kept.carddavUrl)
    }

    @Test
    fun a_use_switched_off_stores_no_server() {
        val mailOnly = SetupUses.chosen(offer(CALDAV, CARDDAV), true, off, off, null)
        assertEquals(listOf(MAIL), mailOnly.uses)
        assertEquals("", mailOnly.caldavUrl + mailOnly.carddavUrl)

        val withoutMail = SetupUses.chosen(offer(CALDAV, CARDDAV), false, on, off, null)
        assertEquals(listOf(CALENDAR), withoutMail.uses)
        assertEquals("", withoutMail.carddavUrl)
    }

    @Test
    fun a_server_not_found_is_the_one_typed() {
        val typed = SetupUses.chosen(
            offer("", ""), true, PickedUse(true, " cal.example.org "), PickedUse(true, "book.example.org"), null,
        )
        assertEquals("cal.example.org", typed.caldavUrl)
        assertEquals("book.example.org", typed.carddavUrl)

        // A box typed into and then switched off stores nothing.
        val dropped = SetupUses.chosen(offer("", ""), true, PickedUse(false, "cal.example.org"), off, null)
        assertEquals("", dropped.caldavUrl)
    }

    @Test
    fun a_card_that_offered_no_choice_leaves_the_uses_to_the_servers() {
        assertNull(SetupUses.chosen(UseOffer.NONE.copy(caldavUrl = CALDAV), null, null, null, null).uses)
    }

    @Test
    fun contacts_are_looked_for_at_a_calendar_typed_in_place_of_one_found() {
        val typed = SetupUses.chosen(offer("", ""), true, PickedUse(true, "cal.example.org"), on, null)
        assertEquals("cal.example.org", typed.caldavUrl)
        assertEquals("", typed.carddavUrl)
        assertEquals(listOf(MAIL, CALENDAR, CONTACTS), typed.uses)
    }

    @Test
    fun colleagues_count_only_beside_contacts() {
        assertEquals(listOf(CONTACTS, COLLEAGUES), SetupUses.chosen(provider(), false, off, on, true).uses)
        assertEquals(listOf(MAIL), SetupUses.chosen(provider(), true, off, off, true).uses)
        // A provider names no server, and none is stored.
        val everything = SetupUses.chosen(provider(), true, on, on, true)
        assertEquals("", everything.caldavUrl + everything.carddavUrl)
    }

    @Test
    fun shared_mailboxes_count_only_beside_mail() {
        val microsoft = UseOffer(
            listOf(MAIL, SHARED_MAILBOXES, CALENDAR, CONTACTS, COLLEAGUES).map { choice(it, true) },
        )
        assertEquals(
            listOf(MAIL, SHARED_MAILBOXES, CALENDAR),
            SetupUses.chosen(microsoft, true, on, off, null, sharedMailboxes = true).uses,
        )
        assertEquals(listOf(CALENDAR), SetupUses.chosen(microsoft, false, on, off, null, true).uses)
        assertEquals(listOf(MAIL), SetupUses.chosen(microsoft, true, off, off, null, false).uses)
    }

    @Test
    fun the_shared_mailboxes_toggle_starts_where_the_core_said_and_counts_when_offered() {
        val toggles = UseToggles(
            UseOffer(listOf(choice(MAIL, true), choice(SHARED_MAILBOXES, true), choice(CALENDAR, true))),
        )
        assertTrue(toggles.sharedMailboxes)
        toggles.calendar = true
        assertEquals(listOf(MAIL, SHARED_MAILBOXES, CALENDAR), toggles.chosen().uses)
        // A personal address is offered none, and its toggle counts for nothing.
        val personal = UseToggles(UseOffer(listOf(choice(MAIL, true), choice(CALENDAR, true))))
        personal.sharedMailboxes = true
        assertEquals(listOf(MAIL, CALENDAR), personal.chosen().uses)
    }

    @Test
    fun a_use_switched_on_with_no_server_asks_for_one() {
        val nothingFound = offer("", "")
        assertEquals(MissingServer.CALENDAR, SetupUses.missing(nothingFound, on, on))
        assertEquals(MissingServer.NONE, SetupUses.missing(nothingFound, PickedUse(true, "cal"), on))
        // Without a calendar, contacts need an address book of their own.
        assertEquals(MissingServer.CONTACTS, SetupUses.missing(nothingFound, off, on))
        assertEquals(MissingServer.NONE, SetupUses.missing(nothingFound, off, off))
        // A server that was found needs nothing typed.
        assertEquals(MissingServer.NONE, SetupUses.missing(offer(CALDAV, ""), on, on))
    }

    @Test
    fun a_manual_sign_in_asks_only_for_what_the_address_is_offered() {
        val chosen = listOf(MAIL, CONTACTS, COLLEAGUES)
        // A personal address is offered no colleagues (docs/accounts.md rule 3).
        val personal = provider().choices.filter { it.capability != COLLEAGUES }
        assertEquals(listOf(MAIL, CONTACTS), SetupUses.allowed(chosen, personal))
        assertEquals(chosen, SetupUses.allowed(chosen, provider().choices))
        assertNull(SetupUses.allowed(null, provider().choices))
    }

    @Test
    fun the_servers_typed_decide_what_a_calendar_and_contacts_account_is_used_for() {
        assertEquals(listOf(CALENDAR, CONTACTS), SetupUses.davUses("cloud.example", ""))
        assertEquals(listOf(CALENDAR, CONTACTS), SetupUses.davUses("cloud.example", "book.example"))
        assertEquals(listOf(CONTACTS), SetupUses.davUses("", "book.example"))
        assertNull(SetupUses.davUses(" ", ""))
    }

    @Test
    fun the_last_use_on_is_held_and_colleagues_follow_contacts() {
        val toggles = UseToggles(offer("", ""))
        // Only mail starts on, so it is the one that cannot be switched off.
        assertTrue(!toggles.canFlip(toggles.mail))
        toggles.calendar = true
        assertTrue(toggles.canFlip(toggles.mail))
        toggles.calendarTyped = ""
        assertTrue(toggles.flagMissing())
        assertEquals(MissingServer.CALENDAR, toggles.flagged)
    }

    @Test
    fun the_mail_servers_follow_the_mail_toggle() {
        val toggles = UseToggles(offer(CALDAV, ""))
        assertTrue(toggles.wantsMail)
        toggles.calendar = true
        toggles.mail = false
        assertTrue(!toggles.wantsMail)
        assertEquals(listOf<AccountCapability>(CALENDAR, CONTACTS), toggles.chosen().uses)
        // A card that offers no choice keeps its mail servers.
        assertTrue(UseToggles(UseOffer.NONE).wantsMail)
    }
}
