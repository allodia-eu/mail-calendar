// The Android client: a Jetpack Compose screen rendering the mailbox-list snapshot
// that the Rust core (mailcal-bindings, via the UniFFI `MailcalApp` object) drives. It
// proves the reactive Rust -> Compose binding end to end, mirroring the macOS SwiftUI
// spike (../../macos/Mailcal.swift): dispatch an intent, the Rust app notifies the
// `Observer` from a runtime thread, Compose hops to the main thread, pulls the immutable
// snapshot, and renders it.
package eu.allodia.mailcal

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent as AndroidIntent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import androidx.activity.compose.setContent
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.AllodiaAccount
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.AnalyticsConsent
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.ConnectivitySnapshot
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.ContactRow
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.EventRow
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailtoPrefill
import uniffi.mailcal_bindings.Observer
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.ReadingSnapshot
import uniffi.mailcal_bindings.ReplyPrompt
import uniffi.mailcal_bindings.SearchHorizon
import uniffi.mailcal_bindings.SharePrefill
import uniffi.mailcal_bindings.UnfiledCopy
import uniffi.mailcal_bindings.SendStatus
import uniffi.mailcal_bindings.SignaturesSnapshot
import uniffi.mailcal_bindings.SnapshotRow
import uniffi.mailcal_bindings.Surface as CoreSurface
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.ThreadMessage
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.WeekStart
import uniffi.mailcal_bindings.SyncProgressSnapshot
import uniffi.mailcal_bindings.SyncSettingsSnapshot
import uniffi.mailcal_bindings.TimeZoneSnapshot
import uniffi.mailcal_bindings.ViewMode
import uniffi.mailcal_bindings.calendarPalette
import uniffi.mailcal_bindings.deviceTimeZone

private const val TAG = "Mailcal"
private const val NOTIFICATION_PERMISSION_REQUEST = 1001

// ConnectionIssue and ExpiredSignIn (the two small data classes refreshConnectivity() below
// builds) live in MainActivityCore.kt beside it.

class MainActivity : AppCompatActivity() {

    // The Compose source of truth, driven by the Rust app.
    internal var rows by mutableStateOf<List<SnapshotRow>>(emptyList())
    internal var mode by mutableStateOf(ViewMode.FLAT)
    // Pagination: the full row count for the current view (rows holds only the visible window)
    // and a guard that coalesces near-end callbacks into one in-flight request.
    internal var totalRows by mutableStateOf(0.toULong())
    internal var loadMorePending = false
    // The configured accounts (sidebar switcher) and the selected one (null = all inboxes),
    // pulled from the mailbox snapshot the Rust core owns.
    internal var accounts by mutableStateOf<List<AccountRow>>(emptyList())
    internal var selectedAccount by mutableStateOf<String?>(null)
    // Every account's sorted folder list, for the navigation drawer. Populated in all view modes.
    internal var accountFolders by mutableStateOf<List<AccountFolderRow>>(emptyList())
    // The All Inboxes badge: every account's Inbox unread, summed. 0 shows none.
    internal var unifiedUnread by mutableStateOf(0u)
    // The selected folder key within the selected account (null = all mail). Pulled with the snapshot.
    internal var selectedFolder by mutableStateOf<String?>(null)
    // How far back the active search looked, or null when the list is not a search.
    internal var searchHorizon by mutableStateOf<SearchHorizon?>(null)
    // The display-timezone setting (active zone + any pending device-zone change). Owned by
    // the Rust core; null until the first settings pull after construction.
    internal var timeZone by mutableStateOf<TimeZoneSnapshot?>(null)

    // The mailbox screen's scroll position and search chrome, held here because the reading view,
    // Settings and the other tabs REPLACE that screen rather than covering it (MailboxUiState.kt).
    // Dispatching through `app` rather than a captured instance, being built before it connects.
    internal val mailbox = MailboxUiState(
        onSearch = { query -> app?.dispatch(Intent.Search(query)) },
        onSetSearchScope = { scope -> app?.dispatch(Intent.SetSearchScope(scope)) },
    )

    // The reading view: the message the user opened (its header, for the screen) and the
    // body snapshot the core fetched + sanitised. Both null when the list is showing.
    internal var openedMessage by mutableStateOf<OpenedMessage?>(null)
    internal var reading by mutableStateOf<ReadingSnapshot?>(null)
    // The conversation the open message belongs to (newest-first) when it was opened from a
    // multi-message thread, else null, drives the reading screen's collapsed strip of the other
    // messages. Cleared alongside `openedMessage` when returning to the list.
    internal var openedConversation by mutableStateOf<List<ThreadMessage>?>(null)

    // The persisted reply/forward quoting settings, the default style, and whether the composer
    // offers a per-message override of it, pulled on connect and on every SETTINGS signal. The
    // style seeds each reply/forward composer; the settings screen edits both.
    internal var quoteSettings by mutableStateOf(
        QuoteSettings(QuoteStyleKind.INDENTED, perMessage = false),
    )

    // The persisted per-direction swipe actions and the default send account, pulled alongside the
    // quote style on every SETTINGS signal. The swipes bind the message rows' gestures; the send
    // account seeds the composer's From dropdown in the unified inbox. Both default until the first
    // pull (both directions Delete, the behaviour before the setting existed).
    internal var swipeSettings by mutableStateOf(
        SwipeSettings(SwipeActionKind.DELETE, SwipeActionKind.DELETE),
    )
    internal var defaultSendAccount by mutableStateOf<String?>(null)

    // The signature library plus each account's two slot assignments, pulled on connect and on
    // every SETTINGS signal. Drives the Signatures settings category; the composer resolves its own
    // signature through the core rather than reading this (docs/signatures.md). Metadata only, a
    // body is fetched one at a time, so opening Settings never drags every embedded logo across the
    // FFI. Null until the first pull.
    internal var signatures by mutableStateOf<SignaturesSnapshot?>(null)

    // The outgoing-send hint, pulled on a SENDING surface change: SENDING while a send is in
    // flight, then the terminal SENT/FAILED which auto-clears back to IDLE after a moment.
    internal var sendStatus by mutableStateOf(SendStatus.IDLE)

    // Background mail-download progress, pulled on a SYNC_PROGRESS surface change: drives the
    // "downloading Y of X" bar while a sync fills in; null/inactive hides it.
    internal var syncProgress by mutableStateOf<SyncProgressSnapshot?>(null)

    // Connectivity, pulled on a CONNECTIVITY surface change: `offline` drives the offline banner;
    // `unreachableAccounts` badges accounts whose server couldn't be reached.
    internal var connectivity by mutableStateOf<ConnectivitySnapshot?>(null)
    // The accounts that can't reach their server right now (while the device is online), each with
    // its friendly email + technical detail, drives the connection-issues banner. Empty while the
    // whole device is offline (the offline banner stands in) or when everything is connected.
    internal var connectionIssues by mutableStateOf<List<ConnectionIssue>>(emptyList())
    // The friendly emails of Microsoft accounts whose calendar is withheld for lack of the calendar
    // OAuth scope (connected before calendar support, or revoked consent), drives the calendar
    // re-auth banner. Reconnecting an account re-runs its sign-in with the calendar scope.
    internal var calendarReauthEmails by mutableStateOf<List<String>>(emptyList())
    // The friendly emails of Microsoft accounts whose mail write/send is withheld for lack of the
    // Mail.ReadWrite / Mail.Send OAuth scopes (connected before those scopes, or revoked consent):
    // drives the mail re-auth banner. Reconnecting an account re-runs its sign-in with the full
    // scope set, so it clears this and any calendar prompt at once.
    internal var mailReauthEmails by mutableStateOf<List<String>>(emptyList())
    // Accounts whose stored sign-in the server has stopped accepting, an expired or revoked OAuth
    // grant, or a password it now refuses. Nothing about the account syncs and retrying never
    // helps, so this drives its own banner rather than the connection-issues one (the core already
    // keeps such an account out of the unreachable list).
    internal var signInExpired by mutableStateOf<List<ExpiredSignIn>>(emptyList())
    // Retains the registered default-network callback so it isn't garbage-collected.
    internal var networkCallback: Any? = null
    // The per-account sync-behaviour settings (fetch depth, push vs. poll, watched folders),
    // pulled on a SETTINGS surface change; feeds the Accounts section of the Settings screen.
    // `showingSettings` swaps the unified Settings screen in over the mailbox, like the reading
    // view.
    internal var syncSettings by mutableStateOf<SyncSettingsSnapshot?>(null)
    // Widened from private: read/written from MainActivityScreen.kt, MainActivitySettingsTab.kt
    // and MainActivityMailboxTab.kt, the extracted MainScreen branches.
    internal var showingSettings by mutableStateOf(false)

    // Which Settings category a shortcut asked for, or null for the hub. Held beside
    // `showingSettings` rather than folded into it: "is Settings open" and "which page of it" are
    // two questions, and one nullable enum answering both makes every reader prove they cannot
    // disagree.
    internal var settingsCategory by mutableStateOf<SettingsCategory?>(null)
    // The Diagnostics screen (Settings → Diagnostics), swapped in over Settings. Both flags stay
    // true while it shows, so its branch is checked first and its Back returns to Settings.
    internal var showingDiagnostics by mutableStateOf(false)

    // Which top-level surface is showing. An enum rather than a flag per destination: with three
    // of them, booleans admit states that cannot exist and every reader has to prove they don't.
    internal var destination by mutableStateOf(AppDestination.MAIL)
    // The destination the app OPENED on, and the floor system back walks down to: from anywhere,
    // back unwinds what is open and then returns here, and only a press made here closes the app.
    // Set once at launch beside `destination` and never moved afterwards, switching tabs must not
    // change where back leads, or "back closes the app" would depend on how the user got here.
    internal var homeDestination = AppDestination.MAIL
    // The contacts list, one row per unified PERSON (the core merges cards sharing an address,
    // across accounts), pulled on a CONTACTS surface change and when switching to the tab.
    internal var contacts by mutableStateOf<List<ContactRow>>(emptyList())
    // Everything the contacts surface needs to WRITE: the books a create may file into, the open
    // editor, the "which account?" question before it, and the last write's outcome. One holder
    // (MainActivityContacts.kt) rather than five slots here, because they are one feature and
    // they change together.
    internal val contactWrites = ContactWriteState()
    // The calendar agenda rows, pulled from the core on a CALENDAR surface change (and when
    // switching to the calendar tab). Soonest first, as ordered by the engine.
    internal var events by mutableStateOf<List<EventRow>>(emptyList())
    // The calendar GRID is not a snapshot, it's a pull with an argument (`calendarPage(view,
    // anchor, ...)`), because a pager renders three pages at once and one snapshot slot cannot hold
    // three. So CALENDAR is demoted to a cache-invalidation signal: this counter is what changes,
    // and the screen re-keys its page pulls on it. See CalendarScreen.kt.
    internal var calendarVersion by mutableStateOf(0)
    // The result of the user's last calendar create/edit/delete, pulled on a CALENDAR_STATUS signal.
    // Drives the small header badge: a spinner while `Saving`, a check on `Saved`, a tap-to-retry
    // warning on `Failed` (which means "could not confirm", not "rejected", see CalendarWriteStatus).
    internal var calendarWriteStatus by mutableStateOf(CalendarWriteStatus.IDLE)
    // The unanswered "your calendar server could not tell the organiser, shall we email them?"
    // question, pulled on an INVITATION_REPLY signal. Null means there is nothing to ask, which is
    // also how the core says "close the modal": it clears the prompt the moment it is answered.
    internal var replyPrompt by mutableStateOf<ReplyPrompt?>(null)
    internal var unfiledCopy by mutableStateOf<UnfiledCopy?>(null)
    // The calendar colours a user may pick from, the core's palette, fetched once. A client cannot
    // invent one, and Allodia Orange is deliberately absent: it means "action" in this product.
    internal val calendarPalette: List<String> by lazy { calendarPalette() }
    // The persisted display preferences the core owns: first day of the week, the 12/24-hour clock
    // (mail AND calendar), the light/dark appearance, and the calendar's default horizon. Pulled on
    // every SETTINGS signal. Seeded with the core's own defaults so the very first frame agrees
    // with it.
    internal var displaySettings by mutableStateOf(
        DisplaySettings(
            WeekStart.MONDAY,
            TimeFormat.TWENTY_FOUR_HOUR,
            Appearance.SYSTEM,
            12u,
            CalendarLayout.WEEK,
        ),
    )

    // The appearance the app is PAINTED in. Its own state rather than a read of `displaySettings`,
    // because it is settled in onCreate, before the core exists, so the first frame is already
    // right, and because a debug launch override may deliberately differ from the stored choice.
    // A pick in Settings sets both this and the core's copy.
    internal var appearance by mutableStateOf(Appearance.SYSTEM)

    // The Rust app, connected on a background thread (the IMAP login blocks, so it must
    // not run on the main thread): null while connecting, set on success. `connectError`
    // carries the failure message if the connect failed.
    internal var app by mutableStateOf<MailcalApp?>(null)
    internal var connectError by mutableStateOf<String?>(null)
    // True on first run (no account stored): show the full-screen setup form. Set once at
    // launch from the stored configs, NOT derived from the live `accounts` list, which is
    // empty until the first sync completes and would otherwise flash the form on every
    // returning launch. Cleared when the first account is added.
    internal var needsSetup by mutableStateOf(false)
    // True while the user is adding another account (the setup form shown over the running app).
    internal var addingAccount by mutableStateOf(false)
    // Whether the usage-statistics question is settled, pulled once at connect. `asked == false`
    // puts the welcome screen up, it is the first thing a new user sees, ahead of setup. Null
    // until the core answers; the welcome screen is not shown on a guess.
    internal var analyticsConsent by mutableStateOf<AnalyticsConsent?>(null)
    // One-shot guard for the MAILCAL_SHOWCASE_SCREEN=reply / =invitation driver, which fires on
    // the first mailbox reload that carries the target row.
    internal var didDriveShowcaseOpen = false
    // A failed add-account connect, shown on the setup form (distinct from the launch-time
    // `connectError`).
    internal var addError by mutableStateOf<String?>(null)
    // True while an IMAP account's Connect is in flight (the blocking login + first sync runs off
    // the main thread), so the Connect button shows a spinner and is disabled.
    internal var isConnecting by mutableStateOf(false)
    // The opaque begin→complete handle for an in-flight Microsoft sign-in, held while the
    // browser is open until the redirect comes back to onNewIntent. Not persisted.
    internal var pendingMicrosoftLogin: String? = null
    // The equivalent handle for an in-flight Google sign-in, held between beginGoogleLogin and the
    // redirect back to onNewIntent. Separate from the Microsoft handle so the two flows can't
    // clobber each other. Not persisted.
    internal var pendingGoogleLogin: String? = null
    // The equivalent handle for an in-flight JMAP sign-in. Separate again, so three concurrent
    // flows can't clobber each other. Not persisted, it carries the PKCE verifier.
    internal var pendingJmapLogin: String? = null
    // The account being signed back in, when the pending JMAP flow is a RE-authentication (the
    // expired-sign-in banner) rather than a first sign-in from the setup form. Both come back
    // through the same redirect scheme, and only this tells the two apart, null means "a new
    // account". Set and cleared alongside `pendingJmapLogin`.
    internal var pendingJmapReauthAccount: String? = null
    // The equivalent handle for an in-flight Allodia account sign-in. Separate again. Not
    // persisted, it carries the PKCE verifier.
    internal var pendingAllodiaSignIn: String? = null
    // A notification deep-link (accountId + messageKey) waiting to be opened once the Rust app
    // has finished connecting. Set from the launch intent (cold start) or onNewIntent (resumed);
    // drained and cleared in connect() once `app` is ready.
    internal var pendingNotificationOpen: Pair<String, String>? = null
    // The composer prefill from a mail link (`mailto:`), set from the launch intent (cold start)
    // or onNewIntent (resumed) and cleared when the composer closes. Compose state, not a plain
    // field: the mailbox screen opens its composer by observing it. It needs no connect() drain
    // like the deep-link above, parsing is pure and account-free, and the mailbox (which hosts
    // the composer) is only composed once the core is up anyway.
    internal var pendingMailto by mutableStateOf<MailtoPrefill?>(null)
    // The same, for a share (docs/os-integration.md): set once its files have been staged.
    internal var pendingShare by mutableStateOf<SharePrefill?>(null)
    // True while a Microsoft sign-in is running (browser + token exchange), so the form shows
    // progress instead of looking idle.
    internal var signingInMicrosoft by mutableStateOf(false)
    // The same, for an in-flight Google sign-in.
    internal var signingInGoogle by mutableStateOf(false)
    // The same, for an in-flight JMAP sign-in, set while discovery + registration run and the
    // browser is open.
    internal var signingInJmap by mutableStateOf(false)
    // The same, for an in-flight Allodia account sign-in, set while the metadata read and the
    // browser hop run, and again from the redirect until the grant is stored.
    internal var signingInAllodia by mutableStateOf(false)
    // Whether that hop has outlasted the first-run card's threshold, so the card owes the person a
    // way back (docs/onboarding.md). Only that card reads it: the settings card is reached from an
    // app somebody is already using, and dismissing the tab is its way out.
    internal var allodiaSignInSlow by mutableStateOf(false)
    // Which hop is the current one. A stale escape timer, or a browser open posted from a metadata
    // read that finished after the person gave up, must not land on the attempt that replaced it.
    internal var allodiaAttempt = 0L
    // Who is signed in to an Allodia account, or null. Seeded at connect from what boot restored
    // and re-read after every sign-in or sign-out; it is a local read that never asks the service.
    internal var allodiaAccount by mutableStateOf<AllodiaAccount?>(null)
    // The last Allodia sign-in or sign-out failure, in the service's own words, or null. Cleared
    // when a new attempt starts.
    internal var allodiaFailure by mutableStateOf<String?>(null)
    // What the person's other devices have to say about their mail accounts. Empty until a pass
    // has run, which is not the same as a pass that found nothing.
    internal var allodiaSync by mutableStateOf(AllodiaSyncState())
    // How each account is shared with the other devices, keyed by account id, what the
    // per-account three-position control draws. A local read; it never asks the service.
    internal var accountsSyncMode by
        mutableStateOf<Map<String, AllodiaAccountSyncMode>>(emptyMap())
    // Whether this launch may sync its account list at all. False in a dev-account launch, whose
    // accounts are canned harness ones: sending those up would put a harness mailbox on the
    // developer's own phone.
    internal var allodiaSyncAllowed = false
    // The address an offer from another device put into the setup flow, so it opens with the
    // typing done. Empty for every other way in.
    internal var setupStartEmail by mutableStateOf("")
    // The record behind that address when the flow was opened from an offer, so the setup
    // screen takes the route the other device wrote down rather than re-deriving one.
    internal var setupStartOffer by mutableStateOf<AllodiaAccountOffer?>(null)
    // Set from the moment the redirect lands until the account is added: the token exchange is a
    // network round trip, and without this the card renders its ordinary idle button for a second
    // or two while work is plainly happening. A button you can press during a step that is
    // already running is not just untidy, it invites a second attempt.
    internal var completingJmap by mutableStateOf(false)
    internal val mainHandler = Handler(Looper.getMainLooper())

    // Requests POST_NOTIFICATIONS on Android 13+ (auto-granted below 33) so the background sync
    // worker can raise new-mail notifications. Fire-and-forget: the worker re-checks the grant at
    // post time, so a denial simply means no notifications rather than a broken flow.
    //
    // Widened from private: called from MainActivityScreen.kt's MainScreen, the extracted setContent
    // tree.
    internal fun maybeRequestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            ActivityCompat.requestPermissions(
                this,
                arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                NOTIFICATION_PERMISSION_REQUEST,
            )
        }
    }

    // The OS broadcasts ACTION_TIMEZONE_CHANGED when the device zone changes at runtime;
    // we report the new zone to the core so it can offer the change prompt. Registered at
    // runtime (onResume) and unregistered (onPause), never via the manifest. Detection is
    // shared Rust (deviceTimeZone()), region-aware and consistent across all clients.
    private val timeZoneChangeReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: AndroidIntent?) {
            app?.dispatch(Intent.ReportDeviceTimeZone(deviceTimeZone()))
        }
    }

    // Rust calls `surfaceChanged` from an internal runtime thread; hop to the main thread
    // before touching Compose state, then pull the fresh immutable snapshot. Which pull answers
    // which signal is MainActivityObserver.kt's `pullFor`.
    internal val observer = object : Observer {
        override fun surfaceChanged(surface: CoreSurface) {
            mainHandler.post {
                val app = app ?: return@post
                // A pending device-zone change can be signalled while the mailbox is
                // showing (the prompt is an overlay), so refresh the timezone snapshot on
                // every signal regardless of which surface changed.
                timeZone = app.timezoneSettings()
                pullFor(surface, app)
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val plan = prepareBoot()
        setContent { MainScreen(plan.showcase) }
        if (plan.showcase) {
            // Nothing stored, nothing dialed: the seeded in-memory dataset stands in for accounts.
            connectShowcase(ShowcaseMode.seedLocale(this))
        } else {
            // The stored (or dev-override) configs drive the initial connect; each dev mode gets its own
            // isolated store so JMAP/IMAP harness data and real accounts never mix. Same `devMode` that
            // selected the configs picks the subdir, so store isolation always matches the account set.
            connect(plan.configs, dataSubdir = devDataSubdir(plan.devMode))
        }
    }

    override fun onResume() {
        super.onResume()
        logUiInfo("activity resumed")
        // Every sign-in whose browser can close without a callback (Microsoft/Google/Allodia
        // Custom Tabs; JMAP's own-task tab is handled inside, not cleared) resets its spinner if
        // the redirect never came back. MainActivityAccounts.kt, beside the flows it cleans up.
        cancelAbandonedSignIns()
        // Runtime registration (not the manifest): catch OS zone changes while we're visible.
        registerReceiver(timeZoneChangeReceiver, IntentFilter(AndroidIntent.ACTION_TIMEZONE_CHANGED))
        // A zone change while we were paused dropped its broadcast, so re-report the current
        // device zone now to catch anything missed (a no-op until the account has connected).
        app?.dispatch(Intent.ReportDeviceTimeZone(deviceTimeZone()))
    }

    override fun onPause() {
        super.onPause()
        logUiInfo("activity paused")
        unregisterReceiver(timeZoneChangeReceiver)
    }

    override fun onStop() {
        super.onStop()
        logUiInfo("activity stopped")
    }

    override fun onDestroy() {
        logUiInfo(
            "activity destroyed (finishing=$isFinishing, changing_config=$isChangingConfigurations)",
        )
        super.onDestroy()
    }

    // Everything the OS can hand a running app arrives here (singleTask, so a second launch is
    // delivered rather than starting a second instance). The routing itself sits beside the other
    // intent helpers in MainActivityBoot.kt.
    override fun onNewIntent(intent: AndroidIntent) {
        super.onNewIntent(intent)
        setIntent(intent)
        routeNewIntent(intent)
    }

}
