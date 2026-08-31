// Which light/dark appearance a launch paints in.
//
// The choice itself is a CORE setting (docs/settings.md -> General), persisted in preferences.toml
// beside every other display preference, so the clients cannot each invent their own default. It is
// read straight off disk rather than pulled from MailcalApp because the first frame is composed long
// before the core exists: newAccounts opens the engine store and starts dialing on a background
// thread, and a screen painted in the device's scheme until that returns is a visible flash of
// exactly the theme the user said they did not want.
//
// Android has no env vars, so, like MAILCAL_DEV_ACCOUNT and MAILCAL_SHOWCASE, the launch override
// is a string intent extra, read only on a debuggable build:
//   adb shell am start -n eu.allodia.mailcal/.MainActivity -e MAILCAL_APPEARANCE dark
package eu.allodia.mailcal

import android.content.pm.ApplicationInfo
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.storedAppearance

internal object AppearanceMode {
    /**
     * The appearance this launch comes up in: the MAILCAL_APPEARANCE override when it names one,
     * else whatever the core has persisted in [dataDir]. A later pick in Settings wins for the rest
     * of the session, the override decides how a run *starts*, not what the app may do.
     */
    fun atLaunch(activity: MainActivity, dataDir: String): Appearance =
        launchOverride(activity) ?: storedAppearance(dataDir)

    // Null on a release build, so a shipped app cannot have its theme flipped by a stray extra:
    // the same property the dev-account and showcase switches hold.
    private fun launchOverride(activity: MainActivity): Appearance? {
        if ((activity.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) == 0) return null
        return parse(activity.intent?.getStringExtra("MAILCAL_APPEARANCE"))
    }

    // The cross-client spellings, matched literally. Anything else is ignored rather than read as
    // "system": a typo'd override that silently did nothing looks exactly like a working one.
    fun parse(raw: String?): Appearance? = when (raw?.trim()?.lowercase()) {
        "system" -> Appearance.SYSTEM
        "light" -> Appearance.LIGHT
        "dark" -> Appearance.DARK
        else -> null
    }
}
