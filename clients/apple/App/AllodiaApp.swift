// The single multiplatform entry point for macOS, iPhone, and iPadOS. This is the ONE file that
// diverges by platform: macOS needs the AppKit delegate to force a regular (dock + frontmost)
// activation (ported from the old AppEntry.swift), and it is the only platform with more than one
// window; iOS/iPadOS use the scene lifecycle as-is.
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

    /// What every window shares: the one model, and through it the one core.
    ///
    /// It lives here rather than inside `ContentView` because a second model is a second
    /// `MailcalApp`, and that is a second connection to the same SQLite store: two writers, two
    /// caches, and whichever wrote last winning (`docs/reading-window.md`). Hoisting it is what
    /// makes a reading window and a composer window *views* of the running app rather than
    /// instances of it.
    @State private var session = AppSession()

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
            #if os(macOS)
            // The mailbox is what the other windows were opened out of, so they go when it does.
            ContentView(session: session).closesItsWindows(session: session)
            #else
            ContentView(session: session)
            #endif
        }
        #if os(iOS)
        // The periodic background mail sync: iOS launches this handler for the registered
        // BGAppRefreshTask; it runs one bounded pass, raises new-mail notifications, and
        // reschedules. macOS keeps its always-on foreground runtime (docs/background-sync.md).
        .backgroundTask(.appRefresh(backgroundRefreshTaskId)) {
            await handleBackgroundRefresh()
        }
        #endif
        #if os(macOS)
        // **No File ▸ New Window.** SwiftUI offers ⌘N for a `WindowGroup` by default, and a second
        // main window would build a second `ContentView`, a second core and a second connection to
        // the store. The windows this app does open are the two below, and both share the session
        // above (`docs/reading-window.md`).
        .commands { CommandGroup(replacing: .newItem) {} }
        #endif

        #if os(macOS)
        // One message, in a window of its own: opened by double-clicking a row, keyed by the
        // message, so double-clicking it again brings its window forward.
        WindowGroup(id: DesktopWindow.reading, for: ReadingWindowID.self) { $window in
            if let window {
                ReadingWindow(session: session, window: window)
            }
        }
        // A window restored at the next launch has no message behind it: the header and the body
        // both live in this session. It would open blank and close itself, so it is not offered.
        .restorationBehavior(.disabled)

        // One draft, in a window of its own: opened by replying or forwarding from a reading
        // window.
        WindowGroup(id: DesktopWindow.composer, for: ComposerWindowID.self) { $window in
            if let window {
                ComposerWindow(session: session, window: window)
            }
        }
        .restorationBehavior(.disabled)
        #endif
    }
}
