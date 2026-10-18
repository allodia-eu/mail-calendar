// A separate server's browser sign-in (docs/mail-oauth.md rule 13): what returning to the app, a
// second press and leaving the form do to its slot (docs/provider-oauth.md rule 13,
// docs/onboarding.md "Nobody may be stranded on it").
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SeparateServerSignInSlotTest {
    private fun opened(): Pair<SeparateServerSignInSlot, Long> {
        val slot = SeparateServerSignInSlot()
        val attempt = slot.begin()
        assertTrue(slot.opening(attempt, "first"))
        return slot to attempt
    }

    @Test
    fun returning_with_the_tab_dismissed_clears_the_spinner_and_keeps_the_handle() {
        val (slot, attempt) = opened()
        assertEquals(SeparateSignInStep.BROWSER, slot.step)

        assertTrue(slot.resumed())

        assertNull(slot.step)
        assertEquals("first", slot.pending)
        // Finishing in the tab, still open in its own task, completes the same sign-in.
        assertEquals(attempt to "first", slot.redirected())
        assertEquals(SeparateSignInStep.COMPLETING, slot.step)
    }

    @Test
    fun returning_after_the_redirect_landed_leaves_the_spinner() {
        val (slot, _) = opened()
        slot.redirected()

        assertFalse(slot.resumed())
        assertEquals(SeparateSignInStep.COMPLETING, slot.step)
    }

    @Test
    fun returning_before_the_browser_opened_leaves_the_spinner() {
        val slot = SeparateServerSignInSlot()
        slot.begin()

        assertFalse(slot.resumed())
        assertEquals(SeparateSignInStep.STARTING, slot.step)
    }

    @Test
    fun a_second_press_supersedes_the_pending_sign_in() {
        val (slot, first) = opened()
        slot.resumed()

        val second = slot.begin()

        assertNull(slot.pending)
        // The first start's answer, arriving late, opens nothing and says nothing.
        assertFalse(slot.opening(first, "stale"))
        assertFalse(slot.settled(first))
        assertTrue(slot.opening(second, "second"))
        assertEquals(second to "second", slot.redirected())
    }

    @Test
    fun a_retired_start_opens_no_browser_and_its_failure_says_nothing() {
        val slot = SeparateServerSignInSlot()
        val attempt = slot.begin()

        slot.retire()

        assertNull(slot.step)
        assertFalse(slot.opening(attempt, "late"))
        assertFalse(slot.settled(attempt))
        assertNull(slot.redirected())
    }

    @Test
    fun a_completion_landing_after_retirement_is_dropped() {
        val (slot, attempt) = opened()
        slot.redirected()

        slot.retire()

        assertFalse(slot.settled(attempt))
        assertNull(slot.step)
    }

    @Test
    fun a_redirect_with_nothing_armed_is_not_taken() {
        assertNull(SeparateServerSignInSlot().redirected())
    }
}
