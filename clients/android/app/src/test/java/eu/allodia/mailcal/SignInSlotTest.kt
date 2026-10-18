// A browser sign-in whose tab runs in its own task, IMAP's and JMAP's: what returning to the app, a
// second press and leaving the card do to it (docs/provider-oauth.md rule 13, docs/onboarding.md
// "Nobody may be stranded on it").
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SignInSlotTest {
    private fun opened(): Pair<SignInSlot<Unit>, Long> {
        val slot = SignInSlot<Unit>()
        val attempt = slot.begin(Unit)
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
        assertEquals(Landed(attempt, "first", Unit), slot.redirected())
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
        val slot = SignInSlot<Unit>()
        slot.begin(Unit)

        assertFalse(slot.resumed())
        assertTrue(slot.signingIn)
    }

    @Test
    fun a_second_press_replaces_the_pending_sign_in() {
        val (slot, first) = opened()
        slot.resumed()

        val second = slot.begin(Unit)

        assertTrue(slot.signingIn)
        assertNull(slot.pending)
        // The first start's answer, arriving late, opens nothing.
        assertFalse(slot.opening(first, "stale"))
        assertTrue(slot.opening(second, "second"))
        assertEquals(Landed(second, "second", Unit), slot.redirected())
    }

    @Test
    fun leaving_the_card_retires_a_start_still_running() {
        val slot = SignInSlot<Unit>()
        val attempt = slot.begin(Unit)

        slot.retire()

        assertFalse(slot.signingIn)
        assertNull(slot.flow)
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
        assertNull(slot.flow)
    }

    @Test
    fun a_re_authentication_supersedes_one_still_in_flight() {
        val slot = SignInSlot<JmapFlow>()
        val first = slot.begin(JmapFlow.Reauth("a"))

        // Pressed again before the first start answered: no guard turns the press away.
        val second = slot.begin(JmapFlow.Reauth("a"))

        assertTrue(slot.signingIn)
        assertFalse(slot.opening(first, "first"))
        assertTrue(slot.opening(second, "second"))
        assertEquals(Landed<JmapFlow>(second, "second", JmapFlow.Reauth("a")), slot.redirected())
    }

    @Test
    fun the_redirect_completes_the_flow_the_slot_holds_now() {
        val slot = SignInSlot<JmapFlow>()
        val reauth = slot.begin(JmapFlow.Reauth("a"))
        slot.opening(reauth, "reauth")
        slot.resumed()

        // An abandoned re-authentication does not route a new account's redirect to itself.
        val added = slot.begin(JmapFlow.NewAccount)
        slot.opening(added, "added")

        assertEquals(Landed<JmapFlow>(added, "added", JmapFlow.NewAccount), slot.redirected())
    }
}
