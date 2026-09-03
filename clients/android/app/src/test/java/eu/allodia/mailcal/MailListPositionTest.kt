// Where the mailbox list sits, and what it does when mail lands at its head.
//
// The rule that needed writing down: an unchanged head of list is the user coming back from a
// message, not mail arriving. Every effect in the list re-runs on that return, so without a memory
// of what was at the head, the return itself announced new mail (or yanked the list to the top).
//
// Plain JUnit: MailListPosition holds Compose snapshot state, but snapshot state is pure JVM, so
// no Robolectric and no composition is needed here. MailboxReturnTest covers the screen swap.
package eu.allodia.mailcal

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

private const val FIRST = "m:acct-1:1"
private const val NEWER = "m:acct-1:2"

class MailListPositionTest {

    @Test
    fun a_cold_start_is_pinned_to_the_top() {
        val position = MailListPosition()

        assertTrue(position.pinnedToTop)
        // The first rows to arrive are not "new mail" on top of anything, and the list is already
        // where the jump would take it.
        assertTrue(position.headOfList(FIRST))
        assertFalse(position.showNewMailPill)
    }

    @Test
    fun mail_arriving_while_the_user_is_scrolled_down_raises_the_pill_instead_of_jumping() {
        val position = MailListPosition()
        position.headOfList(FIRST)
        position.scrollSettled(index = 12, offset = 30)

        assertFalse(position.pinnedToTop)
        // Not a jump: yanking the list to the top would lose the place the user is reading from.
        assertFalse(position.headOfList(NEWER))
        assertTrue(position.showNewMailPill)
    }

    @Test
    fun an_unchanged_head_of_list_is_not_mail_arriving() {
        val position = MailListPosition()
        position.headOfList(FIRST)
        position.scrollSettled(index = 12, offset = 30)

        assertFalse(position.headOfList(FIRST))
        assertFalse(position.showNewMailPill)
    }

    @Test
    fun reaching_the_top_dismisses_the_pill() {
        val position = MailListPosition()
        position.headOfList(FIRST)
        position.scrollSettled(index = 12, offset = 30)
        position.headOfList(NEWER)
        assertTrue(position.showNewMailPill)

        position.scrollSettled(index = 0, offset = 0)

        assertTrue(position.pinnedToTop)
        assertFalse(position.showNewMailPill)
    }

    @Test
    fun a_row_scrolled_only_part_way_up_is_not_the_top() {
        val position = MailListPosition()

        // Half a row of offset still hides the newest message, so an arrival must not yank the
        // list, and the pill is what says so.
        position.scrollSettled(index = 0, offset = 24)

        assertFalse(position.pinnedToTop)
    }

    @Test
    fun an_empty_list_has_no_head_to_react_to() {
        val position = MailListPosition()

        assertFalse(position.headOfList(null))
        assertFalse(position.showNewMailPill)
    }
}
