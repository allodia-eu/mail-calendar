// Boot-time setup and intent handling, split out of MainActivity.kt: the pre-Compose part of
// onCreate (edge-to-edge, the rotating file log, a cold-start deep-link/mailto extraction, the
// dev-account override, the showcase dataset dispatch) and the two intent readers onNewIntent
// shares with it.
package eu.allodia.mailcal

import android.content.Intent as AndroidIntent
import android.content.pm.ApplicationInfo
import android.net.Uri
import android.os.Build
import androidx.activity.enableEdgeToEdge
import uniffi.mailcal_bindings.MailtoPrefill
import uniffi.mailcal_bindings.isAllodiaAccountConfig
import uniffi.mailcal_bindings.parseMailtoUri

// What onCreate needs after `setContent`, to make the initial connect call: the resolved account
// configs, whether this is a showcase (screenshot) launch, and the decoded dev-account override
// (null on a release build or a `personal`/unset launch).
internal data class MainActivityBootPlan(
    val configs: List<String>,
    val showcase: Boolean,
    val devMode: String?,
)

// Everything onCreate does before handing control to Compose: draws edge-to-edge, opens the
// rotating file log, captures a cold-start notification/mailto deep-link, resolves the dev-account
// override, picks the launch appearance, and (for a showcase launch) drives straight to the
// screenshot's target screen. Returns what the final connect()/connectShowcase() call below
// setContent needs.
internal fun MainActivity.prepareBoot(): MainActivityBootPlan {

        // Draw edge-to-edge on EVERY Android version, not just the ones that force it.
        //
        // From API 35 the platform enforces edge-to-edge for targetSdk >= 35 and ignores
        // `statusBarColor` outright, so the app already ran this way on Android 15+, and the
        // screens are written for it (see SystemBarInsetsTest). Below 35 the platform still
        // painted the status bar itself, using the AppCompat.Light theme's `colorPrimaryDark`:
        // a grey #757575 band. AppTheme meanwhile asks for DARK status-bar icons because the app
        // behind it is white, dark icons on grey, which on a Galaxy Note 20 (API 33) made the
        // clock and battery genuinely hard to read.
        //
        // Calling this makes the bars transparent everywhere, so the one appearance decision
        // AppTheme makes is correct on every version instead of only the newest ones.
        enableEdgeToEdge()

        // Open the rotating file log first (under app-private internal storage, the same
        // dataDir the engine store uses) so boot diagnostics are captured from the first line.
        FileLog.init(filesDir.absolutePath)
        logUiInfo(
            "activity created (${Build.MANUFACTURER} ${Build.MODEL}, " +
                "Android ${Build.VERSION.RELEASE}, SDK ${Build.VERSION.SDK_INT})",
        )

        // A cold-start from a new-mail notification carries the account + message to open.
        // Store it so connect() can drain it once the Rust app is ready.
        pendingNotificationOpen = notificationDeepLink(intent)
        // A cold start from a tapped mail link (`mailto:`). Assigned directly rather than through
        // openMailLink(): nothing is on screen yet to clear.
        pendingMailto = mailLinkPrefill(intent)

        // Resolve every stored account's config from the OS secure store and connect them all
        // over one engine. With nothing stored yet (first run) the app comes up account-less
        // and we render the in-app setup form rather than reading any plaintext seed file. The
        // engine store lives in app-private internal storage.
        // A debug launch may override the accounts to target the local Stalwart harness instead of
        // the stored (personal) accounts. Decode the override mode once here (debug builds only;
        // null in normal/release use), it drives both the injected harness configs and the
        // isolated store subdir at connect() below, so the two never disagree. When active, the
        // engine store goes in a separate subdir so test data never mixes with real accounts.
        val devMode = if ((applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0) {
            intent?.getStringExtra("MAILCAL_DEV_ACCOUNT")
        } else {
            null
        }
        // Dev/verification only: point the autodetect JMAP probe at the local Stalwart
        // harness (which the typed domain, e.g. test.local, can't resolve). The core reads
        // this env var under debug/dev-harness builds only. Pair with `adb reverse tcp:28080`.
        if ((applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0) {
            intent?.getStringExtra("MAILCAL_AUTODETECT_WELL_KNOWN_BASE")?.let { base ->
                android.system.Os.setenv("MAILCAL_AUTODETECT_WELL_KNOWN_BASE", base, true)
            }
        }
        val devConfigs = devConfigs(devMode)
        // Before setContent, so the first frame is already in the chosen scheme rather than the
        // device's, the core is minutes of network away and this is one small file on disk.
        appearance = AppearanceMode.atLaunch(this, engineDataDir(devDataSubdir(devMode)))
        val showcase = ShowcaseMode.isOn(this)
        val configs = when {
            showcase -> emptyList()
            // A dev launch connects its canned harness account, plus the one stored entry that is
            // not a mail account (devCarriedOverConfigs), so an Allodia sign-in made in this mode
            // is still there at the next launch.
            devConfigs != null -> devConfigs + devCarriedOverConfigs(SecureStore.configs(this))
            else -> SecureStore.configs(this)
        }
        if (!showcase) logUiInfo("boot loaded ${configs.size} account config(s)")
        // The showcase seeds its own two accounts, so it never shows the setup form, unless the
        // screenshot being taken is *of* that form.
        //
        // "No MAIL account" rather than "nothing stored": the Allodia grant shares this store
        // under a reserved id, so somebody who signs in on the first-run screen and quits before
        // adding a mailbox would otherwise be met at the next launch by an empty inbox and no way
        // back to setup. The core routes that entry out before anything reads it as a mailbox;
        // this asks it the same question.
        needsSetup = if (showcase) false else configs.all { isAllodiaAccountConfig(it) }
        if (showcase) {
            // The periodic worker builds its own headless core over the *stored* accounts, so it
            // would sync the developer's real mail and could raise a real new-mail notification:
            // sender and subject and all, over the screenshot. Application.onCreate enqueued it
            // before it could know this was a showcase launch; cancel it now.
            MailSyncScheduler.cancel(this)
            when (ShowcaseMode.screen(this)) {
                ShowcaseScreen.SETTINGS -> showingSettings = true
                // One arm for all five: opening the form is the whole drive. Which step the
                // documentation screens land on is decided inside AccountSetupFlow, from
                // `ShowcaseMode.setupSeed` and the core's scripted detection, so this never has
                // to know, and can never disagree with, what the app would really show.
                ShowcaseScreen.ADD_ACCOUNT, ShowcaseScreen.SETUP_EMAIL,
                ShowcaseScreen.SETUP_DETECTED, ShowcaseScreen.SETUP_UNTRUSTED,
                ShowcaseScreen.SETUP_MANUAL,
                -> addingAccount = true
                // The grid opens on today's week and pulls its own page from the seeded core, so the
                // flag is all it needs here; connectShowcase kicks the calendar sync once connected.
                // Opening ON the calendar makes it home, so back closes the app from there rather
                // than detouring through a mailbox the run never showed.
                ShowcaseScreen.CALENDAR -> {
                    destination = AppDestination.CALENDAR
                    homeDestination = AppDestination.CALENDAR
                }
                // Both need a loaded row before they can open anything, so they are driven from
                // the mailbox reload (driveShowcaseOpenIfReady) rather than from here.
                ShowcaseScreen.LIST, ShowcaseScreen.REPLY, ShowcaseScreen.INVITATION -> {}
            }
        }

    return MainActivityBootPlan(configs, showcase, devMode)
}

// Extracts the composer prefill from a mail-link launch, or null when this intent is not one
// (the common case, every OAuth redirect also arrives as ACTION_VIEW). `MailtoLaunch` gates
// the action + scheme; the shared core parses the URI, so the header allowlist and the
// injection defences are identical on every platform (docs/composer-security.md, Gate 12).
internal fun mailLinkPrefill(intent: AndroidIntent?): MailtoPrefill? {
    val data = intent?.data ?: return null
    if (!MailtoLaunch.carriesMailLink(intent.action, data.scheme)) {
        return null
    }
    // The URI is message content end to end, recipients, subject, body. Record THAT one
    // arrived and nothing of what it said (docs/logging.md).
    return parseMailtoUri(data.toString())?.also { logUiInfo("mail link received") }
}

// Opens the composer on a mail link that arrived while the app was running. The mailbox hosts
// the composer, so whatever was in front of it is dismissed first, otherwise the composer is
// mounted behind the reading view, the calendar, Contacts or Settings and the tap looks like it
// did nothing. `homeDestination` is deliberately left where it was: a mail link is not the user
// choosing which surface the app opens on, so system back must still walk down to the floor
// they picked. A composer already open is also left alone: `pendingMailto` only seeds a freshly
// opened one, so a link can never discard a draft the user is in the middle of.
internal fun MainActivity.openMailLink(prefill: MailtoPrefill) {
    openedMessage = null
    openedConversation = null
    showingDiagnostics = false
    showingSettings = false
    destination = AppDestination.MAIL
    pendingMailto = prefill
}

// Extracts a notification deep-link from an intent, or returns null if the intent was not
// produced by a per-message notification tap. Both extras must be present and non-blank.
internal fun notificationDeepLink(intent: AndroidIntent?): Pair<String, String>? {
    val accountId = intent?.getStringExtra(MailNotifier.EXTRA_ACCOUNT_ID)?.takeIf { it.isNotBlank() }
    val messageKey = intent?.getStringExtra(MailNotifier.EXTRA_MESSAGE_KEY)?.takeIf { it.isNotBlank() }
    return if (accountId != null && messageKey != null) Pair(accountId, messageKey) else null
}

// A Microsoft or Google OAuth redirect returns to onNewIntent (singleTask + the manifest
// intent-filters): if [data] is one of our custom-scheme callbacks, complete that sign-in with
// the captured URL, dispatching by the redirect scheme. The decision is a pure function of the
// scheme and the host (OAuthRedirect) so the JVM suite can assert it, in particular that JMAP and
// the Allodia account, which share the application-id scheme, are told apart by their host.
internal fun MainActivity.completeOAuthRedirect(data: Uri) {
    when (
        OAuthRedirect.of(
            scheme = data.scheme,
            host = data.host,
            googleScheme = GoogleOAuthConfig.REDIRECT_SCHEME,
            microsoftScheme = MicrosoftOAuthConfig.REDIRECT_SCHEME,
            appScheme = JmapOAuthConfig.REDIRECT_SCHEME,
        )
    ) {
        OAuthRedirect.GOOGLE -> completeGoogleLogin(data.toString())
        OAuthRedirect.MICROSOFT -> completeMicrosoftLogin(data.toString())
        OAuthRedirect.ALLODIA -> completeAllodiaSignIn(data.toString())
        OAuthRedirect.IMAP -> completeImapSignIn(data.toString())
        // One host, two flows: a first sign-in from the setup form and a re-authentication
        // from the expired-sign-in banner. `pendingJmapReauthAccount` is what tells them apart.
        OAuthRedirect.JMAP -> when (val account = pendingJmapReauthAccount) {
            null -> completeJmapSignIn(data.toString())
            else -> completeJmapReauth(account, data.toString())
        }
        null -> Unit
    }
}
