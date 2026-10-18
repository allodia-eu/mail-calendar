// The IMAP browser sign-in's slot: what returning to the app, a second press and leaving the card
// do to it (docs/provider-oauth.md rule 13, docs/onboarding.md "Nobody may be stranded on it").
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ImapSignInSlotTest {
    private fun opened(): Pair<ImapSignInSlot, Long> {
        val slot = ImapSignInSlot()
        val attempt = slot.begin()
        assertTrue(slot.opening(attempt, "first"))
        return slot to attempt
    }

    @Test
    fun returning_with_the_tab_dismissed_clears_the_spinner_and_keeps_the_handle() {
        val (slot, attempt) = opened()

        assertTrue(slot.resumed())

        assertFalse(slot.signingIn)
        assertEquals("first", slot.pending)
        // Finishing in the tab, still open in its own task, completes the same sign-in.
        assertEquals(attempt to "first", slot.redirected())
        assertTrue(slot.signingIn)
    }

    @Test
    fun returning_after_the_redirect_landed_leaves_the_spinner() {
        val (slot, _) = opened()
        slot.redirected()

        assertFalse(slot.resumed())

        assertTrue(slot.signingIn)
        assertNull(slot.pending)
    }

    @Test
    fun returning_before_the_browser_opened_leaves_the_spinner() {
        val slot = ImapSignInSlot()
        slot.begin()

        assertFalse(slot.resumed())
        assertTrue(slot.signingIn)
    }

    @Test
    fun a_second_press_replaces_the_pending_sign_in() {
        val (slot, first) = opened()
        slot.resumed()

        val second = slot.begin()

        assertTrue(slot.signingIn)
        assertNull(slot.pending)
        // The first start's answer, arriving late, opens nothing.
        assertFalse(slot.opening(first, "stale"))
        assertTrue(slot.opening(second, "second"))
        assertEquals(second to "second", slot.redirected())
    }

    @Test
    fun leaving_the_card_retires_a_start_still_running() {
        val slot = ImapSignInSlot()
        val attempt = slot.begin()

        slot.retire()

        assertFalse(slot.signingIn)
        assertFalse(slot.opening(attempt, "late"))
        assertNull(slot.redirected())
        // Nor is anything said about a failure it meets afterwards.
        assertFalse(slot.settled(attempt))
    }

    @Test
    fun leaving_while_the_code_is_exchanged_drops_its_answer() {
        val (slot, attempt) = opened()
        slot.redirected()

        slot.retire()

        assertFalse(slot.settled(attempt))
        assertFalse(slot.signingIn)
    }

    @Test
    fun an_exchange_that_answers_ends_the_sign_in() {
        val (slot, attempt) = opened()
        slot.redirected()

        assertTrue(slot.settled(attempt))
        assertFalse(slot.signingIn)
    }
}
