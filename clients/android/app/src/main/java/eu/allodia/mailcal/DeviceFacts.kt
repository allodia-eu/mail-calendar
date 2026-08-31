package eu.allodia.mailcal

import android.content.Context
import android.content.res.Configuration
import android.os.Build
import uniffi.mailcal_bindings.DeviceClass
import uniffi.mailcal_bindings.DeviceInfo
import uniffi.mailcal_bindings.Platform
import java.util.Locale

/**
 * The device facts every `MailcalApp` constructor reports to the core (`docs/analytics.md`).
 *
 * Two things to keep in mind when touching this:
 *
 * 1. **Report raw; the core coarsens.** We hand over the full OS version (`15`) and the host's own
 *    locale tag (`nl-NL`); the core reduces them to a major and a language it ships before anything
 *    crosses the wire. One tested reduction rule in Rust, not five per-platform reimplementations:
 *    and no client can widen the payload by reporting something more precise than was asked for.
 * 2. **Nothing here is sent unless the user opted in.** These facts are handed to the core at
 *    construction regardless, but the core mints no identifier and sends no event until consent is
 *    given. Building this value is not "collecting" anything.
 *
 * We deliberately do **not** report `Build.MODEL` / `Build.MANUFACTURER`. A raw model string is the
 * strongest identifier an otherwise low-entropy payload could carry, and the Play Console already
 * reports exact models to us for free. Only the phone/tablet form factor is derived, from the screen
 * size, no model string is read at all.
 *
 * Used by both the foreground Activity and the background-sync worker, so it is an `object` with no
 * state, like [CoreLogger].
 */
object DeviceFacts {
    fun of(context: Context): DeviceInfo = DeviceInfo(
        platform = Platform.ANDROID,
        osVersion = Build.VERSION.RELEASE ?: "",
        deviceClass = deviceClass(context),
        appVersion = appVersion(context),
        locale = Locale.getDefault().toLanguageTag(),
    )

    /** The conventional Android tablet threshold: a smallest width of 600dp or more. */
    private fun deviceClass(context: Context): DeviceClass {
        val smallestWidthDp = context.resources.configuration.smallestScreenWidthDp
        return if (smallestWidthDp >= TABLET_SMALLEST_WIDTH_DP) {
            DeviceClass.ANDROID_TABLET
        } else if (smallestWidthDp > 0 || smallestWidthDp != Configuration.SMALLEST_SCREEN_WIDTH_DP_UNDEFINED) {
            DeviceClass.ANDROID_PHONE
        } else {
            DeviceClass.UNKNOWN
        }
    }

    private fun appVersion(context: Context): String = runCatching {
        context.packageManager.getPackageInfo(context.packageName, 0).versionName
    }.getOrNull() ?: "0.0.0"

    private const val TABLET_SMALLEST_WIDTH_DP = 600
}
