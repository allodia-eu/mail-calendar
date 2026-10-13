// Each debug launch mode's engine store: a harness mode never shares the real one.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class DevDataSubdirTest {
    @Test
    fun every_harness_mode_has_a_store_of_its_own() {
        // A harness mode on the real store would write harness mail into it, and count as a
        // launch whose accounts may be synced to the person's Allodia account.
        val modes = listOf("stalwart", "stalwart-multi", "stalwart-imap", DEV_LINKED)
        modes.forEach { assertNotNull(it, devDataSubdir(it)) }
        assertEquals(modes.size, modes.map(::devDataSubdir).toSet().size)
    }

    @Test
    fun a_normal_launch_uses_the_real_store() {
        assertNull(devDataSubdir(null))
    }
}
