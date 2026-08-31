// The one log line that makes a deferred background pass diagnosable from the log alone
// (docs/background-sync.md, docs/logging.md).
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

private fun conditions(
    minutes: Long? = 15,
    exempt: Boolean = true,
    transport: String = "wifi",
    attempt: Int = 0,
) = PassConditions(minutes, exempt, transport, attempt)

class SyncPassDiagnosticsTest {

    @Test
    fun `the gap since the last wake is reported, because that is what the user experiences`() {
        // The whole point. A pass can be perfectly healthy and still deliver mail three hours late,
        // because Doze deferred the *wake*. Without this number the log shows a happy sync and no
        // hint of the delay, and the only way to see it is dumpsys over a cable.
        val line = passConditionsLine(conditions(minutes = 187))

        assertTrue(line, line.contains("+187min since last wake"))
    }

    @Test
    fun `the first wake after install says so rather than claiming a zero gap`() {
        val line = passConditionsLine(conditions(minutes = null))

        assertTrue(line, line.contains("first wake since install"))
        assertFalse("a null gap must not be rendered as +0min", line.contains("+0min"))
    }

    @Test
    fun `the battery exemption is on every line, because it is usually the reason for the gap`() {
        assertTrue(passConditionsLine(conditions(exempt = false)).contains("battery-exempt=false"))
        assertTrue(passConditionsLine(conditions(exempt = true)).contains("battery-exempt=true"))
    }

    @Test
    fun `a retry attempt is called out, since it should never happen`() {
        // A failed pass reports success precisely so WorkManager's backoff never replaces the
        // 15-minute period. A non-zero attempt means that invariant is broken and the cadence is
        // being eaten by an escalating backoff, so it must be visible, not silent.
        assertFalse(passConditionsLine(conditions(attempt = 0)).contains("retry-attempt"))
        assertTrue(passConditionsLine(conditions(attempt = 3)).contains("retry-attempt=3"))
    }

    @Test
    fun `the line carries no content, address, network name or carrier`() {
        // The log is attachable to a support request, so this line is held to the never-log-content
        // rule (docs/logging.md). Transport is deliberately coarse: an SSID or carrier would place
        // the user geographically.
        val line = passConditionsLine(conditions(minutes = 42, exempt = false, transport = "cellular", attempt = 2))

        assertEquals(
            "conditions: +42min since last wake, battery-exempt=false, network=cellular, retry-attempt=2",
            line,
        )
        assertFalse(line.contains("@"))
    }
}
