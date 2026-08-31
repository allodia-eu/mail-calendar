package eu.allodia.mailcal

import android.app.usage.UsageStatsManager
import android.content.Context
import android.net.ConnectivityManager
import android.os.PowerManager
import android.util.Log

// Why this file exists, and why it only *observes*.
//
// The core holds exactly one connectivity bit, fed by `onAvailable`/`onLost`. Those describe the
// DEVICE's default network, which, on a five-day production log from a Galaxy S24, never once went
// away. What went away was this app's *permission to use it*: within 30s of every `activity
// stopped`, established IMAP sockets died with ECONNABORTED (os error 103) and every reconnect
// failed at `getaddrinfo` with EAI_NODATA. 227 doomed DNS lookups, 100% of them while backgrounded,
// 0% while foregrounded, and not one line of the log said "offline", because by the only signal we
// had, we weren't.
//
// So the watch loop's `await_online` gate, whose own doc comment says it exists to stop "the
// every-15s DNS-failure storm", produced 100% of that storm. It could not fire: its input is a fact
// about the device, and the failure is a fact about the uid.
//
// `onBlockedStatusChanged` is the signal we never asked for: it reports when THIS UID's access to a
// network is blocked or unblocked by firewall rules, Doze, App Standby, battery saver, Data Saver.
// If it fires within seconds of backgrounding, it is the missing input for both the retry gate and
// the re-dial trigger, and the fix is to split one boolean into two facts.
//
// This file deliberately does NOT dispatch anything into the core. It is the measurement that
// decides the design, and a measurement that also changes behaviour measures the change. In
// particular it must not raise the offline banner: the user is not looking at the app while we are
// blocked, and a `blocked` bit leaking into the banner means a resume can flash "offline" over
// perfect Wi-Fi.
//
// Only the boolean overload is public. `onBlockedStatusChanged(Network, int blockedReasons)` with
// the BLOCKED_REASON_DOZE / _APP_STANDBY / _BATTERY_SAVER constants exists on API 31+ but is
// @SystemApi, overriding it fails to compile, which is why the power context below is logged
// beside every transition: it is the only way left to infer *which* mechanism acted on us, and an
// App Standby bucket at 23:00 is not the bucket at 05:00.
//
// Two things this cannot tell us, both of which the log will show by their absence:
//   * If the process is FROZEN (the cached-app freezer SIGSTOPs it), no callback is delivered, but
//     we are not making DNS calls either, so there is no storm to explain.
//   * If something outside the uid firewall kills us (Samsung's own deep-sleep, a doze
//     maintenance-window radio power-down), `blocked` stays false and the drops keep coming. That
//     is the result that says the fix cannot rest on this callback alone.
//
// A run with the app battery-exempt proves nothing here: the power-save allowlist skips both
// FIREWALL_CHAIN_DOZABLE and FIREWALL_CHAIN_STANDBY, so Android would not block us, there would be
// no drops, and the null result would read exactly like a pass.

private const val NET_TARGET = "android-net"

// Teed to Logcat as well as the file log, deliberately. `adb exec-out run-as … cat files/logs/app.log`
// what `scripts/dev/logs.sh android --dump` uses, fails on a release build with "package not
// debuggable", so on the build that actually ships, the on-device file is reachable only through the
// in-app Diagnostics share. Logcat has no such restriction, which makes `adb logcat -s Mailcal` the
// only way to confirm in seconds that this instrumentation fires at all, rather than discovering
// tomorrow that a whole night measured nothing. Same tag the rest of the client uses, so one filter
// catches everything.
private const val NET_LOG_TAG = "Mailcal"

private fun net(message: String) {
    Log.i(NET_LOG_TAG, message)
    FileLog.append("INFO", NET_TARGET, message)
}

// The power-management context, so a `blocked` line can be interpreted rather than just counted.
// All three are queryable for one's own package with no permission.
internal fun powerContext(context: Context): String {
    val power = context.getSystemService(PowerManager::class.java)
    val exempt = power?.isIgnoringBatteryOptimizations(context.packageName)
    val deviceIdle = power?.isDeviceIdleMode
    val bucket = runCatching {
        when (context.getSystemService(UsageStatsManager::class.java)?.appStandbyBucket) {
            UsageStatsManager.STANDBY_BUCKET_ACTIVE -> "active"
            UsageStatsManager.STANDBY_BUCKET_WORKING_SET -> "working-set"
            UsageStatsManager.STANDBY_BUCKET_FREQUENT -> "frequent"
            UsageStatsManager.STANDBY_BUCKET_RARE -> "rare"
            UsageStatsManager.STANDBY_BUCKET_RESTRICTED -> "restricted"
            else -> "unknown"
        }
    }.getOrDefault("unavailable")
    return "battery-exempt=$exempt, device-idle=$deviceIdle, standby-bucket=$bucket"
}

// Installed on the existing default-network callback (see observeNetworkReachability). Reports the
// uid's blocked status and the power context, and nothing else.
internal fun logNetworkBlocked(context: Context, blocked: Boolean) {
    net(
        "network for this app is ${if (blocked) "BLOCKED" else "usable"} " +
            "(${powerContext(context)})",
    )
}

// The device's own view, logged at each transition so the two facts sit side by side in the log and
// their disagreement is visible rather than inferred. This is the fact we already had; the line
// above is the one we were missing.
internal fun logNetworkReachability(context: Context, reachable: Boolean) {
    val cm = context.getSystemService(ConnectivityManager::class.java)
    val hasDefault = cm?.activeNetwork != null
    net(
        "device network ${if (reachable) "available" else "lost"} " +
            "(default-network-present=$hasDefault, ${powerContext(context)})",
    )
}
