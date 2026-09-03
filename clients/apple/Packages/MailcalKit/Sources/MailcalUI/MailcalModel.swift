// The SwiftUI source of truth and the Rust-driven `Observer` bridge: `SurfaceObserver`
// hands Rust's surface-changed callback to a Sendable stream, and `MailboxModel` pumps
// it on the main actor, dispatching intents into the Rust app and pulling immutable
// snapshots to publish. Split out of Mailcal.swift to keep each file under 500 lines.

import Foundation
import MailcalBindings
import Network
import os
import SwiftUI

/// Bridges the Rust-driven `Observer` callback into a SwiftUI closure. Rust may call it
/// from any thread, so the closure is `@Sendable`.
final class SurfaceObserver: Observer {
    private let onChange: @Sendable (Surface) -> Void
    init(_ onChange: @escaping @Sendable (Surface) -> Void) { self.onChange = onChange }
    func surfaceChanged(surface: Surface) { onChange(surface) }
}

/// Forwards the Rust core's log records (over the FFI `Logger` port) to the unified logging
/// system, so every layer, the shared Rust core, the bindings, and this host, lands in one
/// stream (viewable in Console.app, or `log stream --predicate
/// 'subsystem == "eu.allodia.mailcal"'`). The Apple counterpart of the Windows file Log
/// and Android's android.util.Log forwarders. The core gates by level before crossing the
/// FFI, and its messages carry no sensitive content, so they are logged `.public`.
final class CoreLogger: MailcalBindings.Logger {
    private let osLog = os.Logger(subsystem: Brand.appID, category: "core")
    func log(level: LogLevel, target: String, message: String) {
        let line = "[\(target)] \(message)"
        switch level {
        case .error: osLog.error("\(line, privacy: .public)")
        case .warn: osLog.notice("\(line, privacy: .public)")
        case .info: osLog.info("\(line, privacy: .public)")
        case .debug, .trace: osLog.debug("\(line, privacy: .public)")
        }
        // Also tee to a file, so diagnostics survive a force-quit (e.g. a hang) and are easy
        // to grab; FileLog picks the platform-appropriate Apple data directory.
        let levelText: String
        switch level {
        case .error: levelText = "ERROR"
        case .warn: levelText = "WARN"
        case .info: levelText = "INFO"
        case .debug: levelText = "DEBUG"
        case .trace: levelText = "TRACE"
        }
        FileLog.shared.append(level: levelText, target: target, message: message)
    }
}

/// The SwiftUI source of truth, driven by the Rust app.
///
/// `@Observable`, not `ObservableObject`: observation is **per property**. Under
/// `@Published` every one of these fields shared a single `objectWillChange`, so a mailbox
/// signal, which writes ten of them, invalidated every view bound to the model, the reading
/// pane and the composer included. During a sync that is the whole UI re-rendering for a change
/// to the list. Here a view re-renders only for the properties its body actually read.
@MainActor
@Observable
final class MailboxModel {
    var rows: [SnapshotRow] = []
    var mode: ViewMode = .flat
    /// The configured accounts, for the sidebar switcher.
    var accounts: [AccountRow] = []
    /// The selected account's id, or `nil` for the unified "all inboxes" view.
    var selectedAccount: String?
    /// The selected account's folders.
    ///
    /// **Not what the sidebar's folder tree renders**, that is ``accountFolders``, which carries
    /// every account's tree in every view. Rendering this one is what made the tree empty itself
    /// on All Inboxes (`docs/folder-pane.md`).
    var folders: [FolderRow] = []
    /// Every account's sorted folder list, keyed by account id in ``accounts`` order. Populated in
    /// every view, so the pane never empties.
    var accountFolders: [AccountFolderRow] = []
    /// The All Inboxes badge: every account's Inbox unread, summed. `0` shows none.
    var unifiedUnread: UInt32 = 0
    var selected: String?
    /// How far back the active search looked, or `nil` when the list is not a search, the sync
    /// depth of the accounts its scope covered (`docs/search.md`).
    var searchHorizon: SearchHorizon?
    var events: [EventRow] = []
    /// The contacts list, one row per unified **person**, not per provider card: the engine has
    /// already merged the cards that share an address, across accounts. Pulled on a
    /// `Surface::Contacts` signal and when switching to the tab.
    var contacts: [ContactRow] = []
    /// Which top-level surface is showing. See AppDestination.swift for why this is an enum.
    var destination: AppDestination = .mail
    /// The calendar GRID is not a snapshot, it is a pull with an argument, because a pager renders
    /// three pages at once and one snapshot slot cannot hold three. So `Surface.calendar` is demoted
    /// to a cache-invalidation signal: THIS is what changes, and the views re-key their page pulls on
    /// it. See MailcalModel.Calendar.swift.
    var calendarVersion = 0
    /// The persisted display preferences the core owns: first day of the week, the 12/24-hour clock
    /// (mail AND calendar), the light/dark appearance, and the calendar's default horizon. Seeded
    /// with the core's own defaults so the very first frame agrees with it.
    var displaySettings = DisplaySettings(
        weekStart: .monday, timeFormat: .twentyFourHour, appearance: .system, visibleHours: 12,
        layout: .week
    )
    /// The appearance the app is **painted** in. Settled here, at model construction, rather than
    /// read off `displaySettings`: the core is an engine open and a network dial away, so waiting
    /// for it would paint the first frames in the desktop's scheme and then switch. A debug launch
    /// override may also deliberately differ from the stored choice.
    var appearance: Appearance = AppearanceMode.atLaunch()
    var timezone: TimeZoneSnapshot?
    /// The open message's fetched, sanitised body (the reading view pulls this on a
    /// `Surface::Reading` signal). `nil` until a message is opened.
    var reading: ReadingSnapshot?
    /// The outgoing-send hint (pulled on a `Surface::Sending` signal): `.sending` while a
    /// send is in flight, then the terminal `.sent`/`.failed` which auto-clears to `.idle`.
    var sendStatus: SendStatus = .idle
    /// The most recent calendar write's status (pulled on a `Surface::CalendarStatus` signal):
    /// `.saving` while a create/edit/delete settles, then `.saved` or `.failed`. `.failed` means
    /// "could not confirm the local view", not "your change was rejected", the write reached the
    /// server and a refresh reconciles it. Drives the small header badge.
    var calendarWriteStatus: CalendarWriteStatus = .idle
    /// The most recent contact write's status (pulled on a `Surface::ContactsStatus` signal), on
    /// the same terms: `.failed` means "we could not confirm this saved", never "rejected". Its
    /// own slot rather than the calendar's, so the contacts list does not report a calendar save.
    var contactWriteStatus: ContactWriteStatus = .idle
    /// The unanswered question raised when a calendar server that promised to tell the organizer
    /// reported that it could not (pulled on a `Surface::InvitationReply` signal). Non-`nil`
    /// presents the modal; the core clears it the moment it is answered, so `nil` is also what
    /// closes it, the host never dismisses it on its own.
    var replyPrompt: ReplyPrompt?
    /// The message that went out without a copy reaching Sent; `nil` when there is nothing to ask.
    var unfiledCopy: UnfiledCopy?
    /// Background mail-download progress (pulled on a `Surface::SyncProgress` signal): drives
    /// the "downloading Y of X" bar while a sync fills in; `nil`/inactive hides it.
    var syncProgress: SyncProgressSnapshot?
    /// The per-account synchronisation-behaviour settings (push vs. poll, watched folders),
    /// pulled on a `Surface::Settings` signal. Drives the "New mail" settings screen.
    var syncSettings: SyncSettingsSnapshot?
    /// The reply/forward quoting settings, the default style, and whether the composer offers a
    /// per-message override of it, pulled on a `Surface::Settings` signal. The style seeds a new
    /// reply's composer; both drive the settings screen.
    var quoteSettings = QuoteSettings(style: .indented, perMessage: false)
    /// The app-level default send account's id (pulled on a `Surface::Settings` signal), or
    /// `nil` when the user hasn't chosen one. It decides which account the composer's From
    /// dropdown opens on for a new message in the unified inbox. This is the **stored** id,
    /// which may name an account the user has since removed, resolve it through `sendAccount`.
    var defaultSendAccount: String?
    /// What a leftward and a rightward swipe across a message row do (pulled on a
    /// `Surface::Settings` signal). Both default to `.delete`, the behaviour before the setting.
    var swipeSettings = SwipeSettings(left: .delete, right: .delete)
    /// The signature library and each account's two assignments (pulled on a `Surface::Settings`
    /// signal). Metadata only, a body is fetched one at a time (`signatureHTML`), so a library
    /// of logos never crosses the FFI just to draw a list of names. Drives the Signatures
    /// settings screen and the composer's override picker.
    var signatures = SignaturesSnapshot(signatures: [], accounts: [])
    /// `true` when no account is configured yet, the host shows the full-screen setup form.
    var needsSetup = false
    /// `true` while the user is adding another account (the setup form as a sheet over the
    /// running app).
    var addingAccount = false
    /// The address an account offered by one of the person's other devices is for, so the setup
    /// sheet opens with the typing done. Empty for every other way in.
    var setupStartEmail = ""
    /// The record behind that address, so the sheet takes the route the other device wrote down
    /// rather than re-deriving one. `nil` for every other way in.
    var setupStartOffer: AllodiaAccountOffer?
    /// An offer was pressed *inside* Settings, which is itself a modal here, a sheet on macOS and
    /// a full-screen cover on iOS, and one modal cannot present another. So the press closes
    /// Settings and this carries the intent across the dismissal, where `ContentView` spends it.
    ///
    /// One-shot on purpose: a flag left set would open the setup sheet the next time somebody
    /// merely closed Settings.
    var addAccountWhenSettingsCloses = false
    /// What the person's other devices have to say about their mail accounts. Empty until a pass
    /// has run, which is not the same as a pass that found nothing.
    var allodiaSync = AllodiaSyncState()
    /// How each account is shared with the other devices, keyed by account id, what the
    /// per-account three-position control draws. A local read; it never asks the service.
    var accountsSyncMode: [String: AllodiaAccountSyncMode] = [:]
    /// A setup/connect error to surface on the form (invalid fields or a failed login).
    var setupError: String?
    /// `true` while a Microsoft sign-in is in flight (browser + token exchange + first
    /// sync), so the form shows progress instead of looking dead.
    var microsoftSigningIn = false
    var googleSigningIn = false // Google sign-in in flight; mirrors microsoftSigningIn.
    /// A JMAP *re*-authentication in flight (the expired-sign-in banner's button), so it can show
    /// as busy. The setup form's first sign-in keeps its own local state; this one is on a screen
    /// that outlives the flow.
    var jmapReconnecting = false
    /// `true` while an IMAP account's Connect is in flight (the blocking login + first sync),
    /// so the Connect button shows a spinner and is disabled, an impatient user can't fire it
    /// twice or dismiss the form mid-connect.
    var isConnecting = false
    /// A non-blocking notice that some stored accounts were skipped at launch because their
    /// mail connect failed (a stale password, a server blip); `nil` when every account
    /// connected. Dismissible, the user clears it once they've seen it.
    var accountNotice: String?
    /// Connectivity (pulled on a `Surface::Connectivity` signal): `offline` drives the offline
    /// banner; `unreachableAccounts` badges accounts whose server couldn't be reached.
    var connectivity: ConnectivitySnapshot?
    /// The local MCP (AI assistant access) settings, pulled on a `Surface::Settings` signal and
    /// once at connect. Carries whether a server is actually listening and where, not only what
    /// the user chose, a panel showing the toggle alone would say "on" while nothing is
    /// reachable. `endpoint == nil` means this platform has no server at all, which is how iOS
    /// hides the panel with no `#if` in the view.
    var mcpSettings: McpSettings?
    /// A draft an assistant asked to open in the composer, unsent. Set by `AgentComposerBridge`
    /// on the main actor; the shell watches it, opens the composer, and clears it.
    var pendingAgentDraft: AgentDraftRequest?
    /// Whether the usage-statistics question is settled, pulled once at connect. `asked == false`
    /// puts the welcome screen up, it is the first thing a new user sees, ahead of setup. Nil
    /// until the core answers, so the screen is never shown on a guess.
    var analyticsConsent: AnalyticsConsent?
    var app: MailcalApp?
    /// Holds an in-app browser tab open on one of the account service's own pages.
    ///
    /// The session's presentation context provider is a **weak** reference, so without this the
    /// provider is released the moment the call returns and the browser has nowhere to present.
    @ObservationIgnored var allodiaBrowser: AllodiaSignIn?
    @ObservationIgnored private var pump: Task<Void, Never>?
    // Not `private`: MailcalModel.Connect.swift's `connect()` reads it.
    @ObservationIgnored var observer: SurfaceObserver?
    /// Watches device network reachability; retained for the app's lifetime. Not `private`, see
    /// `observer`: MailcalModel.Connect.swift's `observeNetworkReachability()` sets it.
    @ObservationIgnored var pathMonitor: NWPathMonitor?
    /// Retains the in-flight Microsoft sign-in session for the duration of its async flow.
    private var microsoftSignIn: MicrosoftSignIn?
    // The in-flight Google browser flow (iOS `GoogleSignIn` / macOS `GoogleLoopbackFlow`), retained
    // for the duration of its async flow; internal so MailcalModel.Google.swift can set it.
    var googleBrowserFlow: (any GoogleBrowserFlow)?
    // Not `private`: MailcalModel.Connect.swift's `observeSystemTimeZone()` sets it.
    // `nonisolated(unsafe)` because the nonisolated `deinit` below unregisters it and the token
    // is not `Sendable`. Nothing races for it: the last reference is gone by the time deinit runs.
    @ObservationIgnored nonisolated(unsafe) var timeZoneObserver: NSObjectProtocol?
    private var started = false
    /// The full row count for the current view (set from each snapshot); `rows` holds only
    /// the visible window, so `rows.count < total` means more can be loaded.
    var total: UInt64 = 0
    /// Coalesces the burst of "last row appeared" events into one in-flight `showMore`.
    var loadMorePending = false
    /// Active display zone, defaulting to the device zone before the first pull.
    var activeZone: String { timezone?.active ?? deviceTimeZone() }

    // `@ObservationIgnored` on the three fields below: they are lifetime plumbing no view reads,
    // and `deinit` is nonisolated, observation would make them main-actor accessors it cannot
    // call.
    deinit {
        pump?.cancel()
        pathMonitor?.cancel()
        if let timeZoneObserver {
            NotificationCenter.default.removeObserver(timeZoneObserver)
        }
    }

    func start() {
        guard !started else { return }
        started = true
        // Rust calls `surfaceChanged` from a runtime thread; the observer yields onto a
        // Sendable stream and the main-actor pump below re-renders. This keeps state in
        // Rust and diffing in SwiftUI, with one clean thread hop.
        let (surfaces, continuation) = AsyncStream.makeStream(of: Surface.self)
        let observer = SurfaceObserver { continuation.yield($0) }
        self.observer = observer
        self.pump = Task { [weak self] in
            for await surface in surfaces {
                guard let self else { continue }
                // Render on arrival, including the mailbox list. The rate is capped in the
                // core: `DebouncedObserver` coalesces a sync burst to one signal per 250 ms, on
                // every client. A second window here could only merge signals arriving closer
                // than that, which by construction they do not, so it merged nothing and cost
                // its own delay on every repaint, a tap to mark read included.
                self.reload(surface)
            }
        }
        // Screenshot only: `MAILCAL_SHOWCASE` boots the seeded in-memory showcase dataset, so a
        // store screenshot never touches a real account. `ShowcaseMode.isOn` is hard-`false` in a
        // release build (MailcalModel+Showcase.swift).
        if startShowcaseIfRequested(observer: observer) { return }
        #if DEBUG
        // Dev/verification only: `MAILCAL_DEV_ACCOUNT` (or `MAILCAL_DEMO`) can boot against the
        // local harness / demo provider instead of the real Keychain accounts, into an isolated
        // store. Lives in the `#if DEBUG` extension (MailcalModel+DevAccount.swift), so none of it
        // ships in a release build.
        if startDevAccountIfRequested(observer: observer) { return }
        #endif
        // Credentials live in the Apple Keychain (the OS secure store), never a plaintext
        // file: read every stored account's config and connect them all over one engine.
        // On first run (nothing stored) the app comes up account-less and shows the setup
        // form, which adds the first account via `addAccount`.
        let configs = KeychainStore.configs()
        // First run (no MAIL account stored) shows the full-screen form; set this before
        // connecting so a connect failure's own `needsSetup` is not overwritten.
        //
        // The Allodia grant lives in this same store under a reserved id, so `configs` is not the
        // list of mail accounts and `isEmpty` is not the question. Somebody who signs in on the
        // first-run screen and quits before adding a mailbox would otherwise be met at the next
        // launch by an empty inbox and no way back to setup, the sign-in having, from where they
        // sit, thrown the app into a state they did not ask for. The core routes the entry out
        // before anything reads it as a mailbox; this asks it the same question.
        needsSetup = configs.allSatisfy { isAllodiaAccountConfig(config: $0) }
        connect(configs)
    }

    /// Runs the Microsoft 365 sign-in: asks the core for the authorization URL, opens it in
    /// the browser (ASWebAuthenticationSession, reusing the browser's Microsoft session),
    /// then completes the flow (token exchange + connect) and stores the returned config in
    /// the Keychain. A cancel or failure surfaces as `setupError` and keeps the form up.
    func signInWithMicrosoft(loginHint: String? = nil) {
        guard let app else {
            setupError = "Could not open the app. Please relaunch."
            return
        }
        do {
            let start = try beginMicrosoftLogin(
                tenant: MicrosoftOAuthConfig.tenant,
                redirectUri: MicrosoftOAuthConfig.redirectURI,
                // The address the user is connecting (from autodetection), so Microsoft targets
                // that account instead of a different signed-in one; nil ⇒ the account picker.
                loginHint: loginHint.flatMap { $0.isEmpty ? nil : $0 }
            )
            let signIn = MicrosoftSignIn()
            microsoftSignIn = signIn
            self.microsoftSigningIn = true
            Task { @MainActor in
                defer {
                    self.microsoftSignIn = nil
                    self.microsoftSigningIn = false
                }
                do {
                    // ASWebAuthenticationSession must start on the main actor.
                    let callbackURL = try await signIn.authorize(
                        authorizationURL: start.authorizationUrl)
                    // The token exchange + folder connect + first mailbox sync are blocking
                    // and can take a while (Graph isn't date-windowed yet), so run them OFF
                    // the main thread, otherwise the whole UI freezes ("not responding")
                    // for the duration. Hop back to the main actor for the UI + Keychain.
                    _ = try await Task.detached(priority: .userInitiated) {
                        try app.completeMicrosoftLogin(
                            pending: start.pending, callbackUrl: callbackURL)
                    }.value
                    self.accountWasAdded()
                } catch MicrosoftSignInError.cancelled {
                    // The user dismissed the browser, not an error; the defer resets the spinner.
                } catch {
                    self.setupError = "\(error)"
                }
            }
        } catch {
            setupError = "\(error)"
        }
    }

}
