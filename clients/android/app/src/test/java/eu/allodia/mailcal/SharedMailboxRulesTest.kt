// How an account's shared mailboxes read on its page (docs/shared-mailboxes.md rules 3 and 8), and
// when the page asks its server again. The same rules the Linux suite pins.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.SharedMailboxEntry
import uniffi.mailcal_bindings.SharedMailboxSupport
import uniffi.mailcal_bindings.SharedMailboxesView

internal fun sharedMailbox(
    name: String,
    address: String? = null,
    selected: Boolean = false,
    listed: Boolean = true,
    handle: String = address ?: name,
) = SharedMailboxEntry(handle, name, address, selected, listed)

internal fun sharedView(support: SharedMailboxSupport, vararg mailboxes: SharedMailboxEntry) =
    SharedMailboxesView(support, mailboxes.toList())

class SharedMailboxRulesTest {
    @Test
    fun a_mailbox_is_named_by_its_address_with_the_server_s_label_when_it_says_more() {
        assertEquals(
            SharedRow("support@example.org", "Support Team", withdrawn = false),
            SharedMailboxRules.row(sharedMailbox("Support Team", "support@example.org")),
        )
        assertEquals(
            SharedRow("support@example.org", null, withdrawn = false),
            SharedMailboxRules.row(sharedMailbox("support@example.org", "support@example.org")),
        )
        // An IMAP folder shared without an address is named by the server's label.
        assertEquals(
            SharedRow("Board", null, withdrawn = false),
            SharedMailboxRules.row(sharedMailbox("Board")),
        )
    }

    @Test
    fun a_chosen_mailbox_the_server_stopped_listing_says_so() {
        val row = SharedMailboxRules.row(
            sharedMailbox("Support Team", "support@example.org", selected = true, listed = false),
        )
        assertTrue(row.withdrawn)
        assertEquals(null, row.label)
    }

    @Test
    fun the_line_beneath_says_what_the_server_answered() {
        assertEquals(SharedNote.NO_MAILBOXES, SharedMailboxRules.note(sharedView(SharedMailboxSupport.LISTED)))
        assertEquals(
            SharedNote.NONE,
            SharedMailboxRules.note(sharedView(SharedMailboxSupport.LISTED, sharedMailbox("a"))),
        )
        assertEquals(SharedNote.BY_ADDRESS, SharedMailboxRules.note(sharedView(SharedMailboxSupport.BY_ADDRESS)))
        assertEquals(
            SharedNote.NOT_CONNECTED,
            SharedMailboxRules.note(sharedView(SharedMailboxSupport.NOT_CONNECTED)),
        )
        // An answer nobody got is not "nothing is shared" (rule 8).
        assertEquals(
            SharedNote.UNREACHABLE,
            SharedMailboxRules.note(sharedView(SharedMailboxSupport.UNREACHABLE, sharedMailbox("a", selected = true))),
        )
    }

    @Test
    fun an_account_that_opens_none_and_holds_none_has_no_section() {
        assertFalse(SharedMailboxRules.shows(null))
        assertFalse(SharedMailboxRules.shows(sharedView(SharedMailboxSupport.UNSUPPORTED)))
        // One chosen while the account could still open it stays until it is switched off.
        assertTrue(
            SharedMailboxRules.shows(sharedView(SharedMailboxSupport.UNSUPPORTED, sharedMailbox("a", selected = true))),
        )
        assertTrue(SharedMailboxRules.shows(sharedView(SharedMailboxSupport.NOT_CONNECTED)))
        assertTrue(SharedMailboxRules.takesAddress(sharedView(SharedMailboxSupport.BY_ADDRESS)))
        assertFalse(SharedMailboxRules.takesAddress(sharedView(SharedMailboxSupport.LISTED)))
    }

    @Test
    fun a_page_asks_again_only_while_the_answer_may_still_change() {
        val mail = listOf(AccountCapability.MAIL)
        val state = SharedMailboxesState()
        assertTrue(state.asksAgain("a", mail))
        assertTrue(state.startAsking("a", mail))
        assertFalse("one ask at a time", state.startAsking("a", mail))
        assertFalse(state.asksAgain("a", mail))
        state.answered("a", sharedView(SharedMailboxSupport.NOT_CONNECTED), state.read())
        assertTrue(state.asksAgain("a", mail))
        state.answered("a", sharedView(SharedMailboxSupport.UNSUPPORTED), state.read())
        assertTrue(state.asksAgain("a", mail))
        // Listing again would be a request to the server on every signal.
        state.answered("a", sharedView(SharedMailboxSupport.LISTED), state.read())
        assertFalse(state.asksAgain("a", mail))
        state.answered("a", sharedView(SharedMailboxSupport.UNREACHABLE), state.read())
        assertFalse(state.asksAgain("a", mail))
        // Switching mail off removes every mailbox chosen, which the last answer still shows on.
        assertTrue(state.asksAgain("a", emptyList()))
        state.answered("a", null, state.read())
        assertTrue(state.asksAgain("a", mail))
    }

    @Test
    fun an_answer_read_before_the_one_shown_does_not_replace_it() {
        val state = SharedMailboxesState()
        val support = sharedMailbox("Support", "support@example.org")
        // The page opens and asks; a switch is made and reads its answer before that ask lands.
        val ask = state.read()
        val afterSwitch = state.read()
        state.answered("a", sharedView(SharedMailboxSupport.LISTED, support.copy(selected = true)), afterSwitch)
        state.answered("a", sharedView(SharedMailboxSupport.LISTED, support), ask)

        assertTrue(state.answers.getValue("a").mailboxes.single().selected)
    }
}
