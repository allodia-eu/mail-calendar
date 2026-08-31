package eu.allodia.mailcal

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.SystemClock
import android.util.Log
import java.lang.ref.WeakReference
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCredentialStore
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.CredentialStoreException
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.SendStatus
import uniffi.mailcal_bindings.ShowcaseLocale
import uniffi.mailcal_bindings.SnapshotRow
import uniffi.mailcal_bindings.ThreadMessage
import uniffi.mailcal_bindings.ThreadRow
import uniffi.mailcal_bindings.deviceTimeZone

private const val TAG = "Mailcal"
private const val RENDER_LOG_MIN_INTERVAL_MS = 5_000L
private const val RENDER_LOG_ROW_STEP = 500L

/// One account that can't reach its server: its display email and the technical detail (its
/// connect error) revealed behind the connection banner's "Details" action.
internal data class ConnectionIssue(val email: String, val detail: String)

/// One account whose stored sign-in the server has stopped accepting: its account id, display
/// email and provider family, which decides the remedy, `MICROSOFT`/`GOOGLE`/`JMAP_OAUTH` re-run
/// their browser sign-in, anything else (a password or pasted-secret JMAP account, or an unknown
/// one) is updated in Settings. The id is what a JMAP re-authentication is addressed to: it
/// re-authorises that account's own persisted grant rather than starting from an address.
internal data class ExpiredSignIn(
    val id: String,
    val email: String,
    val provider: AccountProvider?,
)

private data class RenderLogSample(
    val rowsBucket: Long,
    val totalBucket: Long,
    val mode: String,
    val accounts: Int,
    val atMs: Long,
)

private var lastRenderLog: RenderLogSample? = null

// Whether times render on a 24-hour clock, the core's persisted setting, for the plain (non-
// composable) helpers on the activity. Composables read `LocalUse24Hour` instead.
internal fun MainActivity.uses24HourClock(): Boolean =
    displaySettings.timeFormat == uniffi.mailcal_bindings.TimeFormat.TWENTY_FOUR_HOUR

internal fun logUiInfo(message: String) {
    Log.i(TAG, message)
    FileLog.append("INFO", "android-ui", message)
}

// A failure worth a support engineer's attention. Distinct from [logUiInfo] because the whole
// point of the rotating file log is that a user can attach it to a support request, and a
// failure that only ever reached logcat is invisible the moment the user closes the app, which
// is exactly when they decide to report it. Same privacy rule as everywhere else: reasons and
// identifiers, never message content, addresses, or credentials (docs/logging.md).
internal fun logUiWarn(message: String) {
    Log.w(TAG, message)
    FileLog.append("WARN", "android-ui", message)
}

// The OS secure store, as the core sees it: the *only* way an account's credential is written or
// erased on this device. The core calls it when an account is added, when a refresh token rotates,
// when a grant is re-authorised, and when an account is removed. Uses the application context so
// it outlives the activity.
//
// One class for every provider family: there were three of these, with identical bodies, behind
// three identical FFI ports. That is what made forgetting one cheap, and the cold background
// worker forgot all three for as long as it existed.
//
// Both methods throw rather than swallowing, the core decides what a refused write means, and it
// decides differently depending on what it was doing (a failed add is rolled back; a failed
// rotation cannot be). Reporting `Ok` on a write that did not happen is what the old, return-less
// port made unavoidable.
internal class SecureStoreCredentialStore(context: Context) : AccountCredentialStore {
    private val appContext = context.applicationContext

    override fun persist(accountId: String, configToml: String) {
        try {
            SecureStore.save(appContext, accountId, configToml)
        } catch (e: Exception) {
            throw CredentialStoreException.Store(e.message ?: "the Android Keystore refused the write")
        }
    }

    override fun delete(accountId: String) {
        try {
            SecureStore.remove(appContext, accountId)
        } catch (e: Exception) {
            throw CredentialStoreException.Store(e.message ?: "the Android Keystore refused the erase")
        }
    }
}

// The isolated engine-store subdirectory a debug launch mode uses, or null for a normal one. One
// place, so the store the core opens and the preferences file read ahead of it (the launch
// appearance) can never name different directories.
internal fun devDataSubdir(devMode: String?): String? = when (devMode) {
    "stalwart" -> "dev"
    "stalwart-imap" -> "dev-imap"
    else -> null
}

// Where the engine store and preferences.toml live for this launch, creating the directory if the
// dev subdir does not exist yet.
internal fun MainActivity.engineDataDir(dataSubdir: String?): String =
    if (dataSubdir != null) {
        java.io.File(filesDir, dataSubdir).apply { mkdirs() }.absolutePath
    } else {
        filesDir.absolutePath
    }

// Build the app over every stored account's config off the main thread; IMAP login blocks. A
// non-null dataSubdir isolates the engine store in a subdirectory (used by the dev-account
// override so harness test data never mixes with real accounts).
internal fun MainActivity.connect(configs: List<String>, dataSubdir: String? = null) {
    val activity = this
    val deviceZone = deviceTimeZone()
    val dataDir = engineDataDir(dataSubdir)
    thread(name = "mailcal-connect") {
        try {
            val connected = MailcalApp.newAccounts(
                activity.observer,
                // The core's log sink, shared with the background-sync worker (CoreLogger).
                CoreLogger,
                // INFO by default keeps the rotating file log useful over a long window; the
                // Diagnostics screen's "include more detail" toggle persists DEBUG and this
                // re-applies the choice at boot (the live toggle uses setLogLevel).
                DiagnosticsPrefs.bootLogLevel(activity),
                configs,
                dataDir,
                deviceZone,
                // Reported raw; the core coarsens them, and sends nothing until the user opts in.
                DeviceFacts.of(activity),
                // The secure-store writer, handed over HERE rather than set on the returned core.
                // This constructor starts dialing before it returns, and the first OAuth refresh
                // followed 6 ms later on a real launch, while this thread was still inside the
                // call. The setters this replaces ran in the mainHandler.post below, so whether a
                // rotation half a second later was saved came down to whether the main thread got
                // a turn first (docs/provider-oauth.md rule 5).
                SecureStoreCredentialStore(activity),
            )
            // Where this device remembers what it has synced with the account service. Installed
            // on this thread, before anything can ask for a pass, unlike the credential store
            // above it is not racing a dial, because nothing syncs until somebody asks.
            activity.installAllodiaSyncStore(connected, dataSubdir)
            activity.mainHandler.post {
                activity.app = connected
                // Expose the live core to the background-sync worker so it reuses this instance
                // instead of opening a second store/runtime while the app is merely backgrounded
                // (docs/background-sync.md).
                MailcalApplication.liveCore = WeakReference(connected)
                activity.connectError = null
                // A boot outage no longer dumps raw connect errors into a banner: the failed
                // accounts are kept as placeholders and surfaced by the friendly connection banner.
                // Pull connectivity once now, the outage is seeded before any surface signal fires
                // so the banner + badges show immediately, not only after the next change.
                connected.calendarConnectError()?.let {
                    Log.w(TAG, "calendar (CalDAV) failed to connect: $it")
                }
                connected.dispatch(Intent.ReportDeviceTimeZone(deviceZone))
                activity.timeZone = connected.timezoneSettings()
                activity.syncSettings = connected.syncSettings()
                activity.quoteSettings = connected.quoteSettings()
                activity.swipeSettings = connected.swipeSettings()
                activity.defaultSendAccount = connected.defaultSendAccount()
                activity.displaySettings = connected.displaySettings()
                activity.signatures = connected.signatures()
                // Is the usage-statistics question settled? `asked == false` puts the welcome
                // screen up. Pulled here rather than in onCreate because it needs the core.
                activity.analyticsConsent = connected.analyticsConsent()
                // Who is signed in to an Allodia account, if anyone. A local read of what boot
                // restored from the secure store, it never asks the service, so it costs the
                // connect nothing and Settings has an answer before it is first opened.
                activity.allodiaAccount = connected.allodiaAccount()
                // A dev-account launch connects canned harness accounts, which must never reach
                // the person's real account list.
                activity.allodiaSyncAllowed = dataSubdir == null
                activity.readAccountsSynced()
                // What their other devices have to say. Off the main thread, after the accounts
                // are up: the pass reads the list this connect just built.
                activity.syncAllodiaAccounts()
                // The retention signal, once per launch. A no-op until the user opts in, and on
                // the very first launch the opt-in itself reports the session, so consenting is
                // never a launch we fail to count.
                connected.reportAppOpened()
                activity.observeNetworkReachability()
                activity.refreshConnectivity()
                // Render the primed cached snapshot right away, so the last-synced mail shows
                // instantly, even offline, when the RefreshMail below short-circuits (its rebuild
                // signal is what the list otherwise waits for, and the boot prime's own signal
                // fired before `app` was set).
                activity.reload()
                // Drain any notification deep-link that arrived before the Rust app was ready
                // (cold start from a per-message notification tap).
                activity.pendingNotificationOpen?.let { (accountId, messageKey) ->
                    activity.pendingNotificationOpen = null
                    activity.openMessageByKey(connected, accountId, messageKey)
                }
                connected.dispatch(Intent.RefreshMail)
            }
        } catch (e: Exception) {
            Log.e(TAG, "account connect failed: ${e.message}")
            activity.mainHandler.post {
                activity.connectError = e.message ?: L10n.error_unknown(activity)
            }
        }
    }
}

// Screenshot only: brings the app up on the in-memory showcase dataset instead of the secure store's
// real accounts, so no personal mail can appear in a store screenshot. Nothing is persisted and no
// network is touched, the mailbox and calendar are served from bundled sample content, seeded in the
// language the chrome renders. Gated by `ShowcaseMode.isOn`, which is inert on a release build.
internal fun MainActivity.connectShowcase(locale: ShowcaseLocale) {
    val activity = this
    val deviceZone = deviceTimeZone()
    logUiInfo("MAILCAL_SHOWCASE set, bringing up the in-memory $locale showcase dataset")
    thread(name = "mailcal-showcase") {
        val connected = MailcalApp.newShowcase(
            activity.observer,
            CoreLogger,
            DiagnosticsPrefs.bootLogLevel(activity),
            deviceZone,
            locale,
        )
        activity.mainHandler.post {
            activity.app = connected
            activity.connectError = null
            connected.dispatch(Intent.ReportDeviceTimeZone(deviceZone))
            activity.timeZone = connected.timezoneSettings()
            activity.syncSettings = connected.syncSettings()
            activity.quoteSettings = connected.quoteSettings()
            activity.swipeSettings = connected.swipeSettings()
            activity.defaultSendAccount = connected.defaultSendAccount()
            activity.displaySettings = connected.displaySettings()
            activity.signatures = connected.signatures()
            activity.reload()
            connected.dispatch(Intent.RefreshMail)
            // The calendar screenshot drives to the grid (showingCalendar set pre-setContent); kick
            // its sync here now the core exists, mirroring the real Calendar tap (onShowCalendar) and
            // the Apple/Windows showcase drivers, so the agenda list fills and the grid re-keys.
            if (ShowcaseMode.screen(activity) == ShowcaseScreen.CALENDAR) {
                connected.dispatch(Intent.RefreshCalendar)
            }
        }
    }
}

// Opens a message for reading: records the opened header plus its conversation context (null for
// Opens a message identified only by its account id and provider key (e.g. from a notification
// deep-link). Looks up header fields from the current snapshot rows so the reading-screen header
// is populated if the message is already loaded; falls back to empty strings otherwise (the body
// still arrives via the core's fetch; the header will be blank until a future snapshot includes it).
internal fun MainActivity.openMessageByKey(app: MailcalApp, accountId: String, messageKey: String) {
    val flatRow = rows
        .filterIsInstance<SnapshotRow.Flat>()
        .firstOrNull { it.row.account == accountId && it.row.key == messageKey }
        ?.row
    val opened = OpenedMessage(
        account = accountId,
        key = messageKey,
        subject = flatRow?.subject ?: "",
        from = flatRow?.from ?: "",
        avatar = flatRow?.avatar ?: placeholderAvatar(),
        date = localDateTime(flatRow?.date ?: "", timeZone?.active, uses24HourClock()),
    )
    openMessage(app, opened)
}

// a standalone message, the whole thread, newest-first, when opened from a conversation) and
// asks the core to fetch its body. Carrying the conversation through keeps the reading screen's
// older-messages strip alive as the user opens different messages on the same thread.
internal fun MainActivity.openMessage(
    app: MailcalApp,
    opened: OpenedMessage,
    conversation: List<ThreadMessage>? = null,
) {
    openedMessage = opened
    openedConversation = conversation
    reading = null
    app.dispatch(Intent.OpenMessage(opened.account, opened.key))
}

// Opens a conversation from the list: focuses its latest message (received or sent) in the
// reading screen and carries the whole thread so the older messages show as a collapsed strip
// that opens each on tap (Gmail/Outlook-mobile style). Only real multi-message conversations
// reach here, the core projects a lone message as a flat row, opened via `openMessage`.
internal fun MainActivity.openThread(app: MailcalApp, thread: ThreadRow) {
    val opened = OpenedMessage(
        account = thread.account,
        key = thread.latestKey,
        subject = thread.subject,
        from = thread.latestFrom,
        avatar = thread.avatar,
        date = localDateTime(thread.latestDate, timeZone?.active, uses24HourClock()),
    )
    openMessage(app, opened, thread.messages)
}

// Watches the device's default network and forwards each change to the core
// (ReportNetworkReachable): offline stops it attempting syncs (and raises the banner), online
// triggers a catch-up refresh that also re-dials any dropped provider connections. The callback
// fires with the current state on registration, so the core learns the initial state too; it's
// retained on the activity so it isn't garbage-collected.
internal fun MainActivity.observeNetworkReachability() {
    val activity = this
    val cm = getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager ?: return
    val callback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) = report(true)
        override fun onLost(network: Network) = report(false)

        // Measurement only, deliberately OBSERVE-ONLY, nothing is dispatched into the core.
        // This is the signal the one connectivity bit never carried: whether THIS UID may use the
        // network, as opposed to whether the device has one. See NetworkBlockedLog.kt for why the
        // distinction is the whole bug, and why it must not reach the offline banner.
        override fun onBlockedStatusChanged(network: Network, blocked: Boolean) {
            logNetworkBlocked(activity, blocked)
        }

        private fun report(reachable: Boolean) {
            logNetworkReachability(activity, reachable)
            activity.mainHandler.post {
                activity.app?.dispatch(Intent.ReportNetworkReachable(reachable))
            }
        }
    }
    activity.networkCallback = callback
    cm.registerDefaultNetworkCallback(callback)
    // registerDefaultNetworkCallback fires onAvailable only when a default network exists, and
    // never fires on registration when there is none, so a launch in airplane mode would leave
    // the core thinking it's online. Report the current state once so the offline banner is right.
    val online = cm.getNetworkCapabilities(cm.activeNetwork)
        ?.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) == true
    activity.app?.dispatch(Intent.ReportNetworkReachable(online))
}

// Pulls the current connectivity and resolves each unreachable account to its friendly email +
// technical detail, so the connection-issues banner can name the affected accounts and reveal
// the raw error behind a "Details" action. Called at launch, on a CONNECTIVITY signal, and after
// a reload (so the emails resolve once the switcher list is populated).
internal fun MainActivity.refreshConnectivity() {
    val instance = app ?: return
    val snapshot = instance.connectivity()
    connectivity = snapshot
    connectionIssues = if (snapshot.offline) {
        emptyList()
    } else {
        snapshot.unreachableAccounts.map { id ->
            val email = accounts.firstOrNull { it.id == id }?.email ?: id
            ConnectionIssue(email, instance.connectionDetail(id) ?: email)
        }
    }
    // A standing permission gap, not a connectivity fault, shown regardless of the offline state.
    calendarReauthEmails = snapshot.calendarReauthAccounts.map { id ->
        accounts.firstOrNull { it.id == id }?.email ?: id
    }
    // Likewise the mail write/send permission gap (a refused send or mail action).
    mailReauthEmails = snapshot.mailReauthAccounts.map { id ->
        accounts.firstOrNull { it.id == id }?.email ?: id
    }
    // An account whose stored sign-in the server has stopped accepting (an expired or revoked
    // OAuth grant, a refused password). Not an outage, the core keeps it out of
    // `unreachableAccounts`, so it gets its own banner. Carries the provider because the remedy
    // differs: an OAuth account re-runs its browser sign-in, anything else is fixed in Settings.
    signInExpired = snapshot.signinExpiredAccounts.map { id ->
        ExpiredSignIn(
            id = id,
            email = accounts.firstOrNull { it.id == id }?.email ?: id,
            provider = instance.accountProvider(id),
        )
    }
}

internal fun MainActivity.showMore() {
    val instance = app ?: return
    if (loadMorePending || rows.size.toULong() >= totalRows) return
    loadMorePending = true
    instance.dispatch(Intent.ShowMore)
}

internal fun MainActivity.reload() {
    val instance = app ?: return
    val snapshot = instance.mailboxList()
    rows = snapshot.rows
    totalRows = snapshot.total
    loadMorePending = false
    mode = snapshot.mode
    accounts = snapshot.accounts
    selectedAccount = snapshot.selectedAccount
    accountFolders = snapshot.accountFolders
    unifiedUnread = snapshot.unifiedUnread
    selectedFolder = snapshot.selected
    searchHorizon = snapshot.searchHorizon
    // Re-resolve the connection-issues emails now the switcher list is (re)populated, so an outaged
    // account seeded at boot shows its address rather than its raw id in the banner. Only when
    // there's actually an outage, a healthy reload (the common case, fired repeatedly during a
    // sync) shouldn't spend a connectivity() FFI pull each time; the CONNECTIVITY signal covers it.
    if (connectivity?.unreachableAccounts?.isNotEmpty() == true) {
        refreshConnectivity()
    }
    driveShowcaseOpenIfReady()
    logMailboxRender()
}

// Screenshot only: the two screens that open a designated message once its row has loaded.
//
// `reply` opens the message the sample reply answers, the reading screen then opens its composer
// straight away (initialComposing), pre-filled with the sample text. `invitation` opens the
// meeting invitation and stops there: the card *is* the reading screen, and the core has already
// primed the calendar, so it comes up with its day preview expanded.
//
// Fires once per launch; inert on any other screen.
private fun MainActivity.driveShowcaseOpenIfReady() {
    if (didDriveShowcaseOpen || !ShowcaseMode.isOn(this)) return
    val target = when (ShowcaseMode.screen(this)) {
        ShowcaseScreen.REPLY -> ShowcaseMode.replyTarget(this).let { it.account to it.messageKey }
        ShowcaseScreen.INVITATION ->
            ShowcaseMode.invitationTarget().let { it.account to it.messageKey }
        else -> return
    }
    val instance = app ?: return
    val present = rows.filterIsInstance<SnapshotRow.Flat>()
        .any { it.row.account == target.first && it.row.key == target.second }
    if (!present) return
    didDriveShowcaseOpen = true
    openMessageByKey(instance, target.first, target.second)
}

private fun MainActivity.logMailboxRender() {
    val message = "rendered ${rows.size} of ${totalRows} rows ($mode), ${accounts.size} accounts"
    val now = SystemClock.elapsedRealtime()
    val sample = RenderLogSample(
        rowsBucket = rows.size.toLong() / RENDER_LOG_ROW_STEP,
        totalBucket = totalRows.toLong() / RENDER_LOG_ROW_STEP,
        mode = mode.name,
        accounts = accounts.size,
        atMs = now,
    )
    val previous = lastRenderLog
    val shouldLog = previous == null ||
        previous.rowsBucket != sample.rowsBucket ||
        previous.totalBucket != sample.totalBucket ||
        previous.mode != sample.mode ||
        previous.accounts != sample.accounts ||
        now - previous.atMs >= RENDER_LOG_MIN_INTERVAL_MS
    if (shouldLog) {
        logUiInfo(message)
        lastRenderLog = sample
    } else {
        Log.d(TAG, message)
    }
}

internal fun MainActivity.updateSendStatus(status: SendStatus) {
    sendStatus = status
}
