// The iOS/iPadOS background mail sync (docs/background-sync.md). iOS suspends the process shortly
// after it leaves the foreground, freezing the live IMAP IDLE / poll runtime, so a BGAppRefreshTask
// wakes the app periodically to run one bounded pass and raise new-mail notifications. macOS keeps
// the always-on foreground runtime and needs none of this, hence the whole file is iOS-only.
#if os(iOS)
import BackgroundTasks
import Foundation
import MailcalBindings

/// The BGTaskScheduler identifier for the periodic background mail refresh. Must match the
/// `BGTaskSchedulerPermittedIdentifiers` entry in project.yml (the generated Info.plist) and the
/// `.backgroundTask(.appRefresh:)` handler on the app scene.
public let backgroundRefreshTaskId = "\(Brand.appID).refresh"

/// iOS grants a BGAppRefreshTask only ~30 s of runtime; ask for a little less so the pass finishes
/// and reports before the OS reclaims it. The core clamps this into a sane band regardless.
private let backgroundBudgetSeconds: UInt32 = 25

/// The engine store + preferences dir on iOS (app Application Support/mailcal), the same dir the
/// foreground app uses, so a background pass reads the same store and Keychain accounts.
private func backgroundDataDir() -> String {
    FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("mailcal").path
}

/// A process-global **weak** handle to the foreground live core. A `BGAppRefreshTask` fires in the
/// still-resident (merely *suspended*, not terminated) app process, whose foreground core is alive
/// with the store already open, so the background pass **reuses** that instance instead of opening a
/// SECOND engine store + runtime over the same SQLite file (two write connections, duplicate IMAP).
/// Weak so it clears once the foreground model releases its core; a genuinely terminated app starts a
/// fresh process where this is `nil`, and the handler cold-builds a headless core. Set by
/// `MailcalModel.connect` (iOS only). See docs/background-sync.md.
final class LiveCore: @unchecked Sendable {
    static let shared = LiveCore()
    private let lock = NSLock()
    private weak var app: MailcalApp?

    func set(_ app: MailcalApp?) {
        lock.lock()
        defer { lock.unlock() }
        self.app = app
    }

    func current() -> MailcalApp? {
        lock.lock()
        defer { lock.unlock() }
        return app
    }
}

/// Submits the next app-refresh request (~15 min out). Best-effort, iOS decides the real cadence
/// from usage, and duplicate submits coalesce by identifier. Call at launch and whenever the app
/// enters the background.
public func scheduleBackgroundRefresh() {
    let request = BGAppRefreshTaskRequest(identifier: backgroundRefreshTaskId)
    request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
    do {
        try BGTaskScheduler.shared.submit(request)
    } catch {
        FileLog.shared.append(
            level: "WARN",
            target: "background",
            message: "schedule refresh failed: \(error)"
        )
    }
}

/// Runs one bounded background sync and raises new-mail notifications, then reschedules the next
/// pass. Builds a *headless* core over the same store + Keychain accounts (no standing IDLE/poll
/// watches, one bounded pass, mirroring Android's worker), then drops it. Invoked by the SwiftUI
/// `.backgroundTask(.appRefresh:)` handler on the app scene.
public func handleBackgroundRefresh() async {
    // Line up the next run first, so a throw below never ends the chain.
    scheduleBackgroundRefresh()
    let configs = KeychainStore.configs()
    let dataDir = backgroundDataDir()
    // The FFI pass blocks its thread (the core drives its runtime to completion), so run it off the
    // cooperative pool. The reuse-vs-cold decision happens INSIDE the task so the (non-Sendable) core
    // never crosses an isolation boundary.
    let outcome: BackgroundSyncOutcome? = await Task.detached(priority: .utility) {
        // Still-resident app (suspended, not terminated): reuse the live core rather than opening a
        // second store/runtime over the same DB. run_background_sync is safe on a warm instance (a
        // concurrent poll is absorbed as a Busy skip), and reusing it keeps the notify high-water
        // marks in one place, no cross-core preferences race.
        if let live = LiveCore.shared.current() {
            return live.runBackgroundSync(budgetSeconds: backgroundBudgetSeconds)
        }
        // Cold process (app was terminated): build a headless core just for this pass; ARC frees it
        // when the closure returns.
        guard !configs.isEmpty else { return nil }
        do {
            let core = try MailcalApp.newBackgroundWorker(
                observer: SurfaceObserver { _ in },
                logger: CoreLogger(),
                // The persisted Diagnostics DEBUG opt-in reaches the cold background worker
                // too, a background-sync support session is exactly where the detail matters.
                logLevel: DiagnosticsPrefs.coreLogLevel,
                configs: configs,
                dataDir: dataDir,
                deviceTimezone: deviceTimeZone(),
                deviceInfo: DeviceFacts.current(),
                // The same Keychain writer the app hands its core: this pass refreshes tokens
                // like any other, and the core is released when the closure returns. A rotation
                // with nowhere to go is lost, and against a server that detects a replayed
                // refresh token (Fastmail's ratchet) presenting the superseded one later revokes
                // the whole grant, which is how a real JMAP account died.
                credentialStore: KeychainCredentialStore()
            )
            return core.runBackgroundSync(budgetSeconds: backgroundBudgetSeconds)
        } catch {
            FileLog.shared.append(
                level: "WARN",
                target: "background",
                message: "sync failed: \(error)"
            )
            return nil
        }
    }.value
    if let outcome, NotificationPrefs.enabled {
        await MailNotifier.notifyNewMail(outcome)
    }
}

#if DEBUG
import UserNotifications

/// DEBUG-only: presents new-mail notifications even while the app is foreground, so a live test on
/// the simulator (where the app is the frontmost process, and `BGTaskScheduler` can't background it)
/// can actually see the banner. A release build keeps the default, foreground notifications are
/// suppressed, since you don't notify a user for mail they're already looking at.
final class DebugForegroundNotificationPresenter: NSObject, UNUserNotificationCenterDelegate {
    static let shared = DebugForegroundNotificationPresenter()
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound, .list]
    }
}

/// DEBUG-only: registers a Darwin-notification trigger so a live test can run the background
/// refresh on demand, `BGTaskScheduler` doesn't run on the simulator, so there's otherwise no
/// way to exercise the handler end-to-end there. Post it with:
///   xcrun simctl spawn booted notifyutil -p eu.allodia.mailcal.debugRunSync
/// Never compiled into a release build.
public func installDebugBackgroundTrigger() {
    UNUserNotificationCenter.current().delegate = DebugForegroundNotificationPresenter.shared
    CFNotificationCenterAddObserver(
        CFNotificationCenterGetDarwinNotifyCenter(),
        nil,
        { _, _, _, _, _ in Task { await handleBackgroundRefresh() } },
        "eu.allodia.mailcal.debugRunSync" as CFString,
        nil,
        .deliverImmediately
    )
}
#endif
#endif
