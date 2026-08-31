// Headless runtime proof of the Rust ⇄ Swift UniFFI binding against the REAL account: drive the
// unidirectional loop from a CLI (no SwiftUI/display dependency, so it is deterministic), connect
// the configured IMAP account, sync, and report the INBOX message COUNT only, never subjects, so
// real mail content stays out of logs. The `MailcalVerify` tool target; the AllodiaMail app is the
// visual layer. Reads the account config from ~/.config/mailcal/account.toml (plaintext) rather
// than the Keychain, so the gate stays scriptable.

import Foundation
import MailcalBindings

/// Forwards the Rust-driven `Observer` callback into a closure. Rust may call it from any
/// thread, so the closure is `@Sendable`.
final class PrintObserver: Observer {
    private let onChange: @Sendable (Surface) -> Void
    init(_ onChange: @escaping @Sendable (Surface) -> Void) { self.onChange = onChange }
    func surfaceChanged(surface: Surface) { onChange(surface) }
}

/// Forwards the Rust core's log records to stdout, so the gate also exercises the FFI logging
/// port (the GUI app forwards to the unified log instead).
final class PrintLogger: MailcalBindings.Logger {
    func log(level: LogLevel, target: String, message: String) {
        print("[\(level)] [\(target)] \(message)")
    }
}

/// The secure-store port, for a gate whose account is a plaintext seed file rather than a
/// Keychain item. It prints rather than writing, and prints rather than doing nothing: a
/// rotation reaching here means the seeded account has an OAuth grant, and the gate's operator
/// should know the file on disk is now a generation behind.
final class PrintingCredentialStore: AccountCredentialStore {
    func persist(accountId: String, configToml: String) throws {
        print("[credential-store] the core stored a credential for \(accountId); this gate keeps nothing")
    }

    func delete(accountId: String) throws {
        print("[credential-store] the core erased the credential for \(accountId); this gate keeps nothing")
    }
}

/// The background mail sync now streams every folder concurrently and re-projects the
/// mailbox in waves *after* `refreshMail`'s first signal returns, and its `active` flag
/// even dips to false between waves, so a row count sampled too early is a transient
/// (cached) value the settling sync later corrects, which raced the count assertions below.
/// Settle on the only reliable signal: poll until the projection's row count holds steady
/// AND no sync is in flight for one continuous `quiet` window. Returns the settled count, or
/// nil on timeout.
func waitForStableMailbox(_ app: MailcalApp, quiet: TimeInterval, timeout: TimeInterval) -> Int? {
    let deadline = Date().addingTimeInterval(timeout)
    var last = app.mailboxList().rows.count
    var stableSince = Date()
    while Date() < deadline {
        Thread.sleep(forTimeInterval: 0.2)
        let count = app.mailboxList().rows.count
        if count == last && !app.syncProgress().active {
            if Date().timeIntervalSince(stableSince) >= quiet { return count }
        } else {
            last = count
            stableSince = Date()
        }
    }
    return nil
}

/// Proves the two app-level preferences the composer's From dropdown and the settings screens
/// depend on cross the FFI and round-trip: the default send account (unset until chosen, and
/// clearable) and the per-direction swipe actions (both Delete before the setting is touched, and
/// independently settable). DEMO ONLY, the demo app persists nothing, whereas the real-account
/// mode writes the developer's own preferences file, which a gate must never rewrite.
func verifySendAndSwipeSettings(_ app: MailcalApp) -> Bool {
    // Unset until the user picks one, the core then falls back to the first configured account.
    let initiallyUnset = app.defaultSendAccount() == nil
    app.setDefaultSendAccount(account: "acct-1")
    let stored = app.defaultSendAccount() == "acct-1"
    // Clearing restores "the first configured account" rather than dropping the send.
    app.setDefaultSendAccount(account: nil)
    let cleared = app.defaultSendAccount() == nil

    // Both directions default to Delete: the behaviour before the setting existed.
    var swipe = app.swipeSettings()
    let defaulted = swipe.left == .delete && swipe.right == .delete
    // The two directions are configured independently, setting one leaves the other alone.
    app.setSwipeAction(direction: .left, action: .archive)
    swipe = app.swipeSettings()
    let independent = swipe.left == .archive && swipe.right == .delete

    let ok = initiallyUnset && stored && cleared && defaulted && independent
    print("send/swipe settings: unset=\(initiallyUnset) stored=\(stored) cleared=\(cleared) "
        + "defaulted=\(defaulted) independent=\(independent)")
    return ok
}

@main
struct Verifier {
    static func main() {
        // The shared-Rust time-zone FFI is account-independent, prove it crosses the FFI
        // first: the engine's bundled tzdb list (the picker source, far larger than an OS
        // zone set, including cities a Windows host collapses away) and the region-aware
        // device-zone detection.
        let zones = availableTimeZones()
        print("available zones: \(zones.count)")
        guard zones.count > 100, zones.contains("Europe/Amsterdam") else {
            print("FAIL: availableTimeZones() did not return the engine tzdb")
            exit(1)
        }
        let deviceZone = deviceTimeZone()
        print("device zone: \(deviceZone)")
        guard !deviceZone.isEmpty else {
            print("FAIL: deviceTimeZone() returned empty")
            exit(1)
        }

        let done = DispatchSemaphore(value: 0)
        let app: MailcalApp
        let isDemo = ProcessInfo.processInfo.environment["MAILCAL_DEMO"] == "1"
        if isDemo {
            // CI / no-credentials mode: the in-memory demo provider (a deterministic seeded
            // mailbox, the "Allodia" welcome + a Re: reply thread), so the gate exercises the
            // whole FFI loop without a real account.
            print("mode: demo (in-memory)")
            app = MailcalApp.newDemo(
                observer: PrintObserver { _ in done.signal() },
                logger: PrintLogger(),
                logLevel: .info,
                deviceTimezone: deviceZone
            )
        } else {
            let home = FileManager.default.homeDirectoryForCurrentUser
            let seedPath = home.appendingPathComponent(".config/mailcal/account.toml").path
            let dataDir = home.appendingPathComponent(".local/share/mailcal").path
            // The headless gate reads the config content from the plaintext file directly
            // (the GUI app uses the Keychain); both pass the TOML content to newAccounts.
            guard let configToml = try? String(contentsOfFile: seedPath, encoding: .utf8) else {
                print("FAIL: could not read the account config at \(seedPath)")
                exit(1)
            }
            do {
                app = try MailcalApp.newAccounts(
                    observer: PrintObserver { _ in done.signal() },
                    logger: PrintLogger(),
                    logLevel: .info,
                    configs: [configToml],
                    dataDir: dataDir,
                    deviceTimezone: deviceZone,
                    // The gate links MailcalBindings only, not MailcalUI, so it cannot reach
                    // `DeviceFacts`, and does not need to. Analytics is off in any build with no
                    // relay baked in, and unconsented in every build, so these values are inert.
                    // They are here to exercise the FFI record, which is the gate's whole job.
                    deviceInfo: DeviceInfo(
                        platform: .macos,
                        osVersion: "0",
                        deviceClass: .unknown,
                        appVersion: "0.0.0",
                        locale: "en"
                    ),
                    // This gate's account comes from a plaintext seed file, not the Keychain, so
                    // there is nothing for it to write back to. It still passes a store because
                    // the constructor requires one, which is the property being kept.
                    credentialStore: PrintingCredentialStore()
                )
            } catch {
                print("FAIL: could not open the account: \(error)")
                exit(1)
            }
        }
        print("connected; before sync: \(app.mailboxList().rows.count) rows")

        // The composing/swipe settings the composer and settings screens read. Demo only (see above).
        var settingsOk = true
        if isDemo {
            settingsOk = verifySendAndSwipeSettings(app)
            // The setters signal Surface.Settings; drain so the waits below pair with their own work.
            while done.wait(timeout: .now()) == .success {}
        }

        // Fire-and-forget; the observer signals when the Rust sync completes.
        app.dispatch(intent: .refreshMail)
        if done.wait(timeout: .now() + 30) == .timedOut {
            print("FAIL: the sync did not complete within 30s")
            exit(1)
        }
        // The concurrent background sync re-projects the mailbox in waves after this first
        // signal; settle on a steady row count so the view-toggle steps below sample frozen
        // data (see waitForStableMailbox), then drain the surface-change backlog that piled
        // up during the sync so each wait that follows pairs with a fresh re-projection.
        // Counts only, no subjects, so live mail content never lands in the transcript.
        guard let flat = waitForStableMailbox(app, quiet: 2.0, timeout: 30) else {
            print("FAIL: the mailbox did not settle within 30s")
            exit(1)
        }
        while done.wait(timeout: .now()) == .success {}
        print("flat view: \(flat) messages")

        // Toggle to threaded and confirm the re-projection crosses the FFI.
        app.dispatch(intent: .setViewMode(mode: .threaded))
        if done.wait(timeout: .now() + 15) == .timedOut {
            print("FAIL: the threaded re-projection did not arrive within 15s")
            exit(1)
        }
        let threaded = app.mailboxList().rows.count
        print("threaded view: \(threaded) conversations")

        // Sync + read the calendar through the FFI (counts only).
        app.dispatch(intent: .refreshCalendar)
        if done.wait(timeout: .now() + 30) == .timedOut {
            print("FAIL: the calendar sync did not complete within 30s")
            exit(1)
        }
        let calendarEvents = app.calendarList().events.count
        print("calendar: \(calendarEvents) events")

        // Ranked full-text search (read-only), drive the search FFI; counts only. A bare
        // term matches the indexed text; "allodia" appears in the account's own mail, so a
        // non-zero hit count proves the FTS query + key-mapping work end to end.
        // Settle the projection rather than waiting on a single surface signal: the mail
        // data is frozen by now, but the generic observer also fires for the calendar sync
        // above, so a bare semaphore wait could pair with the wrong surface change.
        app.dispatch(intent: .search(query: "allodia"))
        guard let searchHits = waitForStableMailbox(app, quiet: 0.6, timeout: 15) else {
            print("FAIL: the search did not settle within 15s")
            exit(1)
        }
        print("search 'allodia': \(searchHits) hits")
        // Clearing search returns to the prior view (here threaded, set above), so over the
        // now-frozen data the restored count settles back to `threaded`.
        app.dispatch(intent: .search(query: nil))
        guard let afterClear = waitForStableMailbox(app, quiet: 0.6, timeout: 5) else {
            print("FAIL: clearing search did not settle within 5s")
            exit(1)
        }
        print("after clear: \(afterClear) rows")

        exit(
            settingsOk && flat > 0 && threaded > 0 && threaded <= flat && searchHits > 0
                && afterClear == threaded ? 0 : 2)
    }
}
