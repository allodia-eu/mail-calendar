// The single multiplatform entry point for macOS, iPhone, and iPadOS. This is the ONE file that
// diverges by platform: macOS needs the AppKit delegate to force a regular (dock + frontmost)
// activation (ported from the old AppEntry.swift); iOS/iPadOS use the scene lifecycle as-is.
import SwiftUI
import MailcalUI
import MailcalBindings

#if os(macOS)
import AppKit

/// Forces a regular (dock + frontmost) activation on macOS.
final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        logAppleLifecycle("app finished launching")
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationWillTerminate(_ notification: Notification) {
        logAppleLifecycle("app will terminate")
    }
}
#endif

@main
struct AllodiaApp: App {
    #if os(macOS)
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    #endif

    // Both platforms arm the crash log here rather than in a delegate: this runs before the scene
    // is built, it is earlier than any AppKit callback, and iOS has no delegate to use instead.
    // CrashLog defers the one half that must not be armed this early, see its comment.
    init() {
        CrashLog.install()
        // The deaths CrashLog cannot narrate, because the process was taken away rather than
        // faulting: a memory-pressure kill, a watchdog kill, a hang. MetricKit reports those at the
        // launch after they happened, so this is armed for the *previous* session, not this one.
        CrashDiagnostics.watchForEndedSessions()
    }

    var body: some Scene {
        WindowGroup(L10n.app_title()) {
            ContentView()
        }
        #if os(iOS)
        // The periodic background mail sync: iOS launches this handler for the registered
        // BGAppRefreshTask; it runs one bounded pass, raises new-mail notifications, and
        // reschedules. macOS keeps its always-on foreground runtime (docs/background-sync.md).
        .backgroundTask(.appRefresh(backgroundRefreshTaskId)) {
            await handleBackgroundRefresh()
        }
        #endif
    }
}
