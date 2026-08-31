// Whether Android will let the background sync keep its schedule (docs/background-sync.md).
//
// Why this exists: the periodic sync is correct and WorkManager honours it, while the phone is
// awake it lands on its 15-minute period to the second. Once the phone is idle in a pocket, Doze
// (and, on Samsung, One UI's app sleeping) defer the job to a maintenance window, and the measured
// cadence on a real device stretches to 1-3 hours, and ~11 hours overnight. Nothing in the worker
// or the scheduler can change that: it is the OS deciding the app does not deserve a wakeup.
//
// The only lever the platform gives an app is asking the user to exempt it. Being on the exemption
// list is not a licence to drain the battery, the pass is still one bounded poll every ~15 minutes
// it just stops the OS from deferring that poll for hours.
package eu.allodia.mailcal

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.PowerManager
import android.provider.Settings

object BatteryOptimization {

    /// Whether the OS currently exempts us from Doze's app-standby deferral. When false, the
    /// background sync still runs, just whenever Android feels like letting it.
    fun isExempt(context: Context): Boolean =
        context.getSystemService(PowerManager::class.java)
            ?.isIgnoringBatteryOptimizations(context.packageName) == true

    /// The system dialog asking the user to allow unrestricted background use.
    ///
    /// The `package:` URI is not optional and not cosmetic: without it the system shows the *list*
    /// of all apps rather than a prompt for this one, and the user is left hunting for us in it.
    fun requestIntent(context: Context): Intent =
        Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
            .setData(Uri.parse("package:${context.packageName}"))
}
