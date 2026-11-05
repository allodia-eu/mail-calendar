// The wait a Nextcloud sign-in is (docs/nextcloud-sign-in.md rule 5): polled until the server
// grants, expires or cannot be read; asked at once when the app comes back; stopped by the surface
// that moved on, and saying why a browser that did not open ended it.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.NextcloudSignInPoll

class NextcloudFlightTest {
    private fun answers(vararg polls: NextcloudSignInPoll): () -> NextcloudSignInPoll {
        val queue = ArrayDeque(polls.toList())
        return { queue.removeFirst() }
    }

    @Test
    fun aGrantEndsTheWaitAfterThePollsThatFoundNobodyYet() {
        val end = NextcloudFlight().await(
            1,
            answers(NextcloudSignInPoll.Waiting, NextcloudSignInPoll.Waiting, NextcloudSignInPoll.Granted("granted")),
        )
        assertTrue(end is NextcloudEnd.Granted)
        assertEquals("granted", (end as NextcloudEnd.Granted).grant)
        assertFalse("a grant is never printed", end.toString().contains("granted"))
    }

    @Test
    fun theAppComingBackAsksAtOnceRatherThanAtTheNextInterval() {
        val flight = NextcloudFlight()
        flight.wake()
        val started = System.nanoTime()
        val end = flight.await(60_000, answers(NextcloudSignInPoll.Waiting, NextcloudSignInPoll.Expired))
        assertEquals(NextcloudEnd.Expired, end)
        assertTrue(System.nanoTime() - started < 30_000_000_000L)
    }

    @Test
    fun aStoppedFlightAsksNothingMoreAndReportsItsFirstStop() {
        val flight = NextcloudFlight()
        flight.stop(NextcloudEnd.Failed(null))
        flight.stop()
        assertEquals(NextcloudEnd.Failed(null), flight.await(1, answers()))
        assertTrue(flight.isStopped)
    }

    @Test
    fun anUnreadableGrantEndsTheSignIn() {
        val garbled = MailcalException.Connect("garbled")
        val end = NextcloudFlight().await(1) { throw garbled }
        assertEquals(NextcloudEnd.Failed(garbled), end)
    }
}
