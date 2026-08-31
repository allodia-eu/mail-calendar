// The battery-optimisation exemption request (docs/background-sync.md).
//
// Worth pinning because both halves of this intent fail *quietly*. A wrong action opens some other
// settings page; a missing `package:` URI opens the full list of every app on the phone instead of
// a prompt for this one, leaving the user to hunt for us in it. Neither throws, and a screenshot of
// "a settings screen appeared" looks like success.
package eu.allodia.mailcal

import android.content.Context
import android.provider.Settings
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class BatteryOptimizationTest {

    private val ctx: Context get() = RuntimeEnvironment.getApplication()

    @Test
    fun `the request targets the exemption prompt, not a settings list`() {
        val intent = BatteryOptimization.requestIntent(ctx)

        assertEquals(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, intent.action)
    }

    @Test
    fun `the request names this package, so the user is prompted rather than sent app-hunting`() {
        val intent = BatteryOptimization.requestIntent(ctx)

        assertEquals("package:${ctx.packageName}", intent.data?.toString())
    }

    @Test
    fun `a fresh install is not exempt, so the prompt is shown`() {
        // The default: Android exempts nothing until the user says so. If this ever returns true
        // out of the box the settings card would hide itself and the cadence would silently stay
        // at Doze's mercy.
        assertEquals(false, BatteryOptimization.isExempt(ctx))
    }
}
