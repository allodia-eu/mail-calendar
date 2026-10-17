// The unread count on the app's icon (docs/app-badge.md): the Dock tile on macOS, the Home Screen
// icon on iOS and iPadOS. One number, the unified Inbox unread the pane's All Accounts › Inbox row
// shows, and nothing at zero.

#if os(macOS)
import AppKit
#else
import UserNotifications
#endif

/// What this process last put on the icon, so a snapshot that leaves the count alone asks the OS
/// for nothing.
@MainActor
struct AppBadge {
    /// `nil` until the first write, so the first snapshot always writes: an iOS badge outlives the
    /// process, and one left standing by an earlier run must be corrected even when the count is
    /// now zero.
    private var shown: UInt32?

    mutating func show(_ unread: UInt32) {
        guard unread != shown else { return }
        shown = unread
        Self.write(unread)
    }

    /// Writes `unread` whether or not it moved, for when the icon may have been changed, or a
    /// write ignored, out of this process's sight.
    mutating func rewrite(_ unread: UInt32) {
        shown = nil
        show(unread)
    }

    /// Writes `unread` to the icon. On iOS the `.badge` permission `MailNotifier` asks for governs
    /// it, and without that permission the OS ignores the write.
    static func write(_ unread: UInt32) {
        #if os(macOS)
        // `NSApp` is unset in a host-less test run.
        NSApp?.dockTile.badgeLabel = label(unread)
        #else
        UNUserNotificationCenter.current().setBadgeCount(Int(unread))
        #endif
    }

    /// The Dock tile's text for `unread`: none at zero.
    nonisolated static func label(_ unread: UInt32) -> String? {
        unread == 0 ? nil : String(unread)
    }
}
