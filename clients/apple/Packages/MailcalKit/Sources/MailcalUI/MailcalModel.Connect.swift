// Opening the engine over the stored accounts, and wiring the OS-level observers the running app
// reacts to. Factored into its own `MailboxModel` extension to keep MailcalModel.swift under the
// 500-line limit (the model is already split this way, see MailcalModel.Actions.swift,
// MailcalModel.Composer.swift, …).

import Foundation
import MailcalBindings
import Network

extension MailboxModel {
    /// Opens the engine over every stored account `configs` and wires the reactive loop.
    /// `newAccounts` blocks briefly on each account's network connect (still on the main
    /// actor). An empty `configs` brings up an account-less app for first-run
    /// setup. On failure it surfaces the error and falls back to the setup form.
    func connect(_ configs: [String], dataDirName: String = DevNamespace.currentDataDirName) {
        guard let observer else { return }
        let dataDir = DevNamespace.storeDirectory(dataDirName)
        // Ensure the store directory exists, a fresh dev store dir won't yet.
        try? FileManager.default.createDirectory(atPath: dataDir, withIntermediateDirectories: true)
        do {
            let app = try MailcalApp.newAccounts(
                observer: observer,
                logger: CoreLogger(),
                // INFO by default keeps the rotating file log useful over a long window; the
                // Diagnostics settings' "include more detail" toggle persists a DEBUG opt-in
                // for a support session, honoured here at boot and live via setLogLevel.
                logLevel: DiagnosticsPrefs.coreLogLevel,
                configs: configs,
                dataDir: dataDir,
                deviceTimezone: deviceTimeZone(),
                // Reported raw; the core coarsens them, and sends nothing until the user opts in.
                deviceInfo: DeviceFacts.current(),
                // The Keychain writer, handed over HERE rather than set on the returned app. This
                // constructor starts dialing before it returns, the first OAuth refresh follows
                // within milliseconds, so there is no "immediately afterwards" early enough to
                // install one (docs/provider-oauth.md rule 5).
                credentialStore: KeychainCredentialStore()
            )
            self.app = app
            // Where this device remembers what it has synced with the account service. Installed
            // before anything can ask for a pass; unlike the Keychain writer above it is not
            // racing a dial, because nothing syncs until somebody asks.
            installAllodiaSyncStore(app)
            // What the person's other devices have to say. Detached, because the pass blocks on
            // the network and nothing on screen waits for it.
            readAccountsSynced()
            Task { await syncAllodiaAccounts() }
            #if os(iOS)
            // Expose the live core to the background-sync task so a BGAppRefreshTask reuses this
            // instance instead of opening a second store/runtime while the app is merely suspended
            // (docs/background-sync.md).
            LiveCore.shared.set(app)
            #endif
            // The agent (MCP) surface. Both calls are what make it exist at all: without an
            // endpoint the core has nowhere to listen, and without the composer port an
            // assistant's `create_draft` reports that this build has no composer. iOS returns
            // nil from `McpEndpoint.path`, so it is excluded by construction rather than by an
            // `#if` here. Setting the endpoint applies the persisted settings, so a user who had
            // it on last session is listening again from this point.
            app.setAgentHostUi(ui: AgentComposerBridge(model: self))
            app.setMcpEndpoint(endpoint: McpEndpoint.path(dataDirName: dataDirName))
            mcpSettings = app.mcpSettings()
            // Pull the per-account sync-behaviour settings so the "New mail" screen opens current.
            syncSettings = app.syncSettings()
            // The swipe actions are read while rendering every row, so mirror them now rather
            // than crossing the FFI per row; the composer's From dropdown wants the stored
            // default send account before the first `Surface::Settings` signal arrives.
            swipeSettings = app.swipeSettings()
            defaultSendAccount = app.defaultSendAccount()
            // The composer resolves the account's signature when it opens, so the library has to
            // be mirrored before the first compose, not on the first `Surface::Settings` signal.
            signatures = app.signatures()
            // Is the usage-statistics question settled? `asked == false` puts the welcome screen up.
            analyticsConsent = app.analyticsConsent()
            // The retention signal, once per launch. A no-op until the user opts in, and on the
            // very first launch the opt-in itself reports the session, so consenting is never a
            // launch we fail to count.
            app.reportAppOpened()
            // A stored account skipped at launch (its mail connect failed) is non-fatal but
            // user-visible: surface it as a dismissible in-app notice, not just a log line.
            if let accountError = app.accountConnectError() {
                accountNotice = L10n.accounts_skipped_notice(details: accountError)
            }
            // A configured-but-failed CalDAV connect is non-fatal (mail still works), so it
            // is otherwise invisible, log it here as the likely empty-calendar cause.
            if let calendarError = app.calendarConnectError() {
                print("[Mailcal] calendar (CalDAV) failed to connect: \(calendarError)")
            }
            // Pull the initial timezone setting (which may already carry a pending change
            // if the stored zone differs from this device's zone), then watch for the OS
            // zone changing while the app runs.
            self.timezone = app.timezoneSettings()
            self.observeSystemTimeZone()
            self.observeNetworkReachability()
            app.dispatch(intent: .refreshMail)
        } catch {
            print("[Mailcal] could not open the accounts: \(error)")
            setupError = L10n.status_connect_failed(error: "\(error)")
            needsSetup = true
        }
    }

    /// Watches for the OS reporting a different time zone (e.g. a laptop changing
    /// regions) and forwards it to the core, which raises a pending change the UI
    /// prompts on. Detection is shared Rust (`deviceTimeZone()`), which reads the OS
    /// fresh and region-aware each call (it resets CoreFoundation's cached system zone
    /// internally), so the reported zone is the real current city.
    private func observeSystemTimeZone() {
        timeZoneObserver = NotificationCenter.default.addObserver(
            forName: Notification.Name.NSSystemTimeZoneDidChange,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            let id = deviceTimeZone()
            Task { @MainActor in self?.app?.dispatch(intent: .reportDeviceTimeZone(id: id)) }
        }
    }

    /// Watches the device's network reachability (`NWPathMonitor`) and forwards each change to
    /// the core: offline stops it attempting syncs (and raises the banner), online triggers a
    /// catch-up refresh that also re-dials any dropped provider connections. The first update
    /// fires on start, so the core learns the initial state without a special-case launch call.
    private func observeNetworkReachability() {
        let monitor = NWPathMonitor()
        self.pathMonitor = monitor
        monitor.pathUpdateHandler = { [weak self] path in
            let reachable = path.status == .satisfied
            Task { @MainActor in
                self?.app?.dispatch(intent: .reportNetworkReachable(reachable: reachable))
            }
        }
        monitor.start(queue: DispatchQueue(label: "\(Brand.appID).network"))
    }
}
