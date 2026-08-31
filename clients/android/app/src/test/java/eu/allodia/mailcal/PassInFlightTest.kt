package eu.allodia.mailcal

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/// The guard that stops two overlapping WorkManager passes each building their own core over the
/// same accounts, two independent refreshers of one credential, which on a ratcheting server is
/// the replay that revokes the grant.
///
/// Its own test because both halves of it fail silently. A guard that never admits anyone means the
/// app stops syncing in the background until it is restarted; a guard that admits everyone means the
/// bug is still there and nothing says so.
class PassInFlightTest {
    @Test
    fun `a second pass is refused while the first holds the slot`() {
        assertTrue("the first pass must be admitted", PassInFlight.claim())
        assertFalse("two passes ran concurrently over one credential", PassInFlight.claim())
        PassInFlight.release()
    }

    @Test
    fun `the slot is reusable once released`() {
        assertTrue(PassInFlight.claim())
        PassInFlight.release()
        assertTrue(
            "a released slot stayed taken, background sync would stop until the app restarts",
            PassInFlight.claim(),
        )
        PassInFlight.release()
    }
}
