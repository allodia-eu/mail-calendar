// Screenshot/showcase mode: the MAILCAL_SHOWCASE launch flag brings the app up on the in-memory
// showcase dataset (two fictional accounts, seeded sample content) instead of the secure store's real
// accounts, so store screenshots need no personal mail. The Play listing wants a screenshot set per
// language, so the flag doubles as the language switch: MAILCAL_SHOWCASE=nl seeds Dutch sample mail to
// sit under Dutch chrome. Nothing is persisted. The Android twin of Windows' Services/ShowcaseMode.cs
// and the Apple client's ShowcaseMode.swift.
//
// Android has no env vars, so, like MAILCAL_DEV_ACCOUNT, the flag is a string intent extra, read
// only on a debuggable build:
//   adb shell am start -n eu.allodia.mailcal/.MainActivity -e MAILCAL_SHOWCASE nl \
//                                                          -e MAILCAL_SHOWCASE_SCREEN reply

package eu.allodia.mailcal

import android.content.pm.ApplicationInfo
import androidx.appcompat.app.AppCompatDelegate
import uniffi.mailcal_bindings.ShowcaseLocale
import uniffi.mailcal_bindings.showcaseInvitation
import uniffi.mailcal_bindings.showcaseLocaleForLanguage
import uniffi.mailcal_bindings.showcaseReply

// Which screen a screenshot run drives to once the mailbox has loaded.
internal enum class ShowcaseScreen(val flag: String) {
    // The message list. Android is single-pane, so nothing is opened over it.
    LIST("list"),

    // The reply composer for the showcase's designated message, pre-filled with sample text.
    REPLY("reply"),

    SETTINGS("settings"),
    ADD_ACCOUNT("add-account"),

    // The calendar grid, opened on today's week.
    CALENDAR("calendar"),

    // The showcase's meeting invitation, opened in the reading screen, the mail-and-calendar
    // surface: Accept / Maybe / Decline over a preview of the day the meeting would land on.
    INVITATION("invitation"),

    // ---- documentation screens (docs/user-docs.md) ----------------------------------------
    //
    // Captured by `showcase.sh android --set docs` for the user guides, not for a store listing.
    // These four are one flow photographed at four moments, and they are driven by a single seam
    // the address to type and whether to run detection, because in a showcase build detection
    // answers from a script keyed on the domain (`MailcalApp.detectAccountSettings`). So the core
    // decides which screen appears, and the client never fakes a result it would not really show.
    //
    // The `mcp-*` doc screens are deliberately absent: agent access is desktop-only by
    // construction (docs/mcp.md), and showcase.sh refuses to ask Android for them.

    // The email-first setup step with an address typed and detection not yet run.
    SETUP_EMAIL("setup-email"),

    // The detected-settings card for a domain that published everything over HTTPS, the
    // trusted, calendar-included happy path.
    SETUP_DETECTED("setup-detected"),

    // The same card for settings only reachable over a plain-HTTP hop: the approval gate a user
    // must tick before any password is sent.
    SETUP_UNTRUSTED("setup-untrusted"),

    // The manual IMAP/SMTP form, reached because nothing was published for the domain.
    SETUP_MANUAL("setup-manual"),
    ;

    companion object {
        fun from(flag: String?): ShowcaseScreen =
            entries.firstOrNull { it.flag == flag?.trim()?.lowercase() } ?: LIST
    }
}

// How a documentation run opens the account-setup flow: the address to prefill, and whether to
// run detection on it straight away.
internal data class ShowcaseSetupSeed(val email: String, val runDetection: Boolean)

// The seed for a screen, or null for every screen that is not part of the setup walkthrough:
// and so for every real launch, where the field starts empty as it should.
//
// The addresses match what the core's detection script answers for (SHOWCASE_TRUSTED_DOMAIN /
// SHOWCASE_UNTRUSTED_DOMAIN in crates/mailcal-bindings/src/autodetect.rs); `northwind.example` is
// also the showcase's own work account, so the walkthrough and the mailbox screenshots tell one
// story. Pure, and shared with the Apple client's ShowcaseMode.setupSeed, a divergence here would
// photograph two different flows under one screenshot id.
internal fun showcaseSetupSeed(screen: ShowcaseScreen): ShowcaseSetupSeed? = when (screen) {
    ShowcaseScreen.SETUP_EMAIL -> ShowcaseSetupSeed("eva@northwind.example", runDetection = false)
    ShowcaseScreen.SETUP_DETECTED -> ShowcaseSetupSeed("eva@northwind.example", runDetection = true)
    ShowcaseScreen.SETUP_UNTRUSTED -> ShowcaseSetupSeed("bram@oldschool.example", runDetection = true)
    ShowcaseScreen.SETUP_MANUAL -> ShowcaseSetupSeed("eva.jansen@example.com", runDetection = true)
    else -> null
}

// Reads the showcase launch extras. Every accessor is inert on a non-debuggable build, so no
// showcase path can run in a shipped build.
internal object ShowcaseMode {
    private val OFF = setOf("", "0", "false", "no", "off")

    // The trimmed, lower-cased MAILCAL_SHOWCASE extra; null when unset or on a release build.
    private fun raw(activity: MainActivity): String? {
        if ((activity.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) == 0) return null
        return activity.intent?.getStringExtra("MAILCAL_SHOWCASE")?.trim()?.lowercase()
    }

    // Whether showcase mode is on, MAILCAL_SHOWCASE set to anything other than 0/false/no/off.
    fun isOn(activity: MainActivity): Boolean = raw(activity)?.let { it !in OFF } ?: false

    // The language the flag pins the app to (any locale the catalog ships, e.g. "de"), or null
    // when it names none (MAILCAL_SHOWCASE=1) and the app's own language choice stands.
    fun languageOverride(activity: MainActivity): String? =
        raw(activity)?.takeIf { it in L10n.LOCALES }

    // The locale to seed the showcase mailbox and calendar in: the pinned language when the flag
    // names one, else whatever the chrome resolved to. Dutch chrome over English mail reads as a
    // broken screenshot, so the sample content always follows the UI. The code -> locale mapping
    // lives in the core, so all three clients seed the same content for the same language.
    fun seedLocale(activity: MainActivity): ShowcaseLocale =
        showcaseLocaleForLanguage(languageOverride(activity) ?: chromeLanguage())

    fun screen(activity: MainActivity): ShowcaseScreen =
        ShowcaseScreen.from(activity.intent?.getStringExtra("MAILCAL_SHOWCASE_SCREEN"))

    // The documentation walkthrough's seed, or null on every non-showcase run.
    fun setupSeed(activity: MainActivity): ShowcaseSetupSeed? =
        if (isOn(activity)) showcaseSetupSeed(screen(activity)) else null

    // The sample reply text to open the composer pre-filled with when replying to the showcase's
    // designated message, else null, every other message, and every non-showcase run, keeps the
    // normal empty composer. Plain text; see docs/composer-security.md Gate 11.
    fun replyText(activity: MainActivity, account: String, key: String): String? {
        if (!isOn(activity)) return null
        val reply = showcaseReply(seedLocale(activity))
        return if (account == reply.account && key == reply.messageKey) reply.text else null
    }

    // The message the reply screenshot replies to.
    fun replyTarget(activity: MainActivity) = showcaseReply(seedLocale(activity))

    // The message the invitation screenshot opens. Locale-free: the seed translates the meeting's
    // words, not its key, so every language opens the same row.
    fun invitationTarget() = showcaseInvitation()

    // The app's effective language: its per-app locale (AppCompat), else the system default.
    //
    // A showcase run pins that per-app locale from outside the app, before launch:
    //   adb shell cmd locale set-app-locales eu.allodia.mailcal --locale nl
    // Setting it from inside onCreate can't work, AppCompat applies a locale change by recreating
    // the Activity, which it won't do for an Activity still being created, so the app would come up
    // blank. `scripts/dev/showcase.sh` sets it before `am start` and clears it afterwards.
    private fun chromeLanguage(): String {
        val tag = AppCompatDelegate.getApplicationLocales().toLanguageTags().ifEmpty {
            java.util.Locale.getDefault().toLanguageTag()
        }.lowercase()
        // The same rule Android's own resource resolution follows: the first catalog locale the
        // tag names, else the base, read off L10n.LOCALES so the two can't drift.
        return L10n.LOCALES.firstOrNull { tag == it || tag.startsWith("$it-") } ?: "en"
    }
}
