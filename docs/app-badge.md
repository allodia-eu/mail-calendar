# The app-icon badge: cross-platform contract

**Scope.** The number on the app's own icon: the Dock tile on macOS, the Home Screen icon on iOS
and iPadOS, the taskbar button on Windows, a dock's launcher on Linux. The counts inside the app
are [`folder-pane.md`](folder-pane.md)'s, and the notifications are
[`background-sync.md`](background-sync.md)'s.

**Principle.** The icon says what the pane says. Someone who sees 12 on the Dock and opens the app
finds 12 on the All Accounts › Inbox row.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **The icon carries the unified Inbox unread**, `MailboxListSnapshot::unified_unread`: the number on the All Accounts › Inbox row ([`folder-pane.md`](folder-pane.md), rule 7). Inbox only, never every folder, and never a count a client works out for itself. | Summing every folder would count Junk and Archive. The core already puts this sum in every snapshot, so a client that computes its own has two numbers that can disagree. |
| 2 | **No badge at zero**, for the reason the pane draws none ([`folder-pane.md`](folder-pane.md), rule 6). | Zero folds together "nothing unread" and "this provider reports no count", and neither has a number worth drawing. |
| 3 | **Nothing else writes it.** A notification carries no badge of its own, and a new-mail pass's total is never shown there. | A pass's total is what has just arrived, not what is unread: a badge reading 3 and then 1 as two passes land says the second pass, and on iOS it stays on the icon until the next one. One number from one source is what keeps rule 1 true. |
| 4 | **It follows the snapshot.** A client writes it whenever a mailbox snapshot carries a count other than the last one it successfully wrote, and always for the first snapshot of a launch. On macOS and iOS it is also written again each time the app comes to the foreground. | The first write corrects a badge an earlier run left behind, including one that should now read nothing. Writing only on a change keeps a sync's burst of snapshots from becoming a burst of calls into the shell. On Apple the icon can change out of the model's sight: a background pass writes it, and a write made before the badge permission was granted in Settings is dropped by the OS. |
| 5 | **A desktop takes the badge off when the app quits; a phone keeps it.** macOS removes a Dock badge with the process. Windows clears it as the main window closes and Linux as the application shuts down. iOS leaves it standing, and its background pass rewrites it from the counts that pass read from each server. | A desktop app that has quit syncs nothing, so its last count only goes stale. An iOS app is never visibly quit, and its background pass is what keeps the number true. |
| 6 | **The platform's own setting turns it off, and the app adds none.** On macOS and iOS the badge is drawn only with the `.badge` permission, which is asked for with notifications; on Windows, the taskbar's "Show badges on taskbar apps"; on Linux, whatever the dock offers. | A switch inside the app would be one more place for the answer to differ from the system's. |
| 7 | **Android draws no count.** Android has no API that puts a number on a launcher icon. Its launcher draws a dot, and on some devices a number, from the notifications the app has posted. That is the launcher's own, and it counts notifications not yet dismissed, not unread mail. | A vendor's private badge API reaches one manufacturer's launcher and is not a platform answer. Setting the notifications' own number to the unread count would put this count on some launchers and leave every other launcher showing something else. |

**Gmail adds nothing to the count.** Its folders report no unread number
([`folder-pane.md`](folder-pane.md), "Known gaps"), so a Gmail Inbox contributes zero to the sum,
and someone whose only account is Gmail sees no badge. The day the engine reads Gmail's count, the
badge follows with no change here.

**The count moves with the folder-list sync.** Reading a message takes it off the badge when the
sync that follows lands, the same moment the pane's count changes, so the two never disagree.

## Per-platform

| Platform | Where | Written by | From | On quit | Turned off by |
|---|---|---|---|---|---|
| macOS | Dock tile | `NSApp.dockTile.badgeLabel` (`AppBadge.swift`) | each mailbox snapshot (`MailcalModel.Reload.swift`) | goes with the process | System Settings → Notifications, the app's badge switch |
| iOS/iPadOS | Home Screen icon | `UNUserNotificationCenter.setBadgeCount` (`AppBadge.swift`) | each mailbox snapshot, and the end of every background pass (`BackgroundSync.swift`) | stays, and the next pass rewrites it | Settings → Notifications, the app's Badges switch |
| Windows | taskbar button | `BadgeNotificationManager` (`Services/AppBadge.cs`), in a packaged build only | each mailbox snapshot (`MailboxModel.Projection.cs`) | cleared as the main window closes (`App.xaml.cs`) | Settings → Personalisation → Taskbar, "Show badges on taskbar apps" |
| Linux | the dock's launcher | Unity `LauncherEntry.Update`, a session-bus broadcast (`ui/launcher_badge.rs`) | each mailbox snapshot, read after every message the window handles (`ui/component.rs`) | cleared on the application's `shutdown` (`main.rs`) | the dock. Ubuntu Dock, KDE Plasma and Plank draw it; GNOME Shell without a dock extension draws nothing |
| Android | none (rule 7) | — | — | — | — |

The Linux signal needs no permission in either package. The Flatpak's bus proxy passes a signal
addressed to no one, and the snap's `desktop` plug, which the GNOME extension adds, allows this
interface and member. In a snap, the dock knows the app by the desktop file snapd installed,
`<instance>_<app>.desktop`, never by the application id.

On Windows the badge needs package identity: `BadgeNotificationManager` refuses every call from a
process without one, however its notifications are registered. Both shipped channels are packages
([`windows-channels.md`](windows-channels.md)), so only the unpackaged dev loop
(`build-and-run.ps1`) draws no badge, and it says so once in its log.

## Where each rule lives

Rules 1 and 2 are the core's: `unified_unread` is summed in
[`view.rs`](../crates/mailcal-viewmodel/src/view.rs) and tested in `folders_and_accounts.rs`
(`mailcal-viewmodel`). Rules 3 to 5 are each client's, and so is the mapping of zero to no badge:
`AppBadgeTests.swift` and `noNotificationSetsTheIconBadge` (`NewMailNoticesTests.swift`) on Apple,
and `launcher_badge.rs`'s own tests on Linux, which also hold the snap's desktop-file id. Nothing
in a Windows test can read the taskbar, so `AppBadge.cs` is checked by eye, in a packaged build
(`package.ps1 -Sign`): open the app with unread mail, read the taskbar button, mark a message
read, and close the window.

## Enforcement

When you change the badge on any client:

1. Write `unified_unread` and nothing else, and clear the badge at zero rather than writing `0`
   where the platform would draw it.
2. Write it from the path that already receives each mailbox snapshot, only when the count moved,
   and always for the first snapshot. Write nothing before the core has published one, or a
   launch clears the badge the last run left until the first snapshot puts it back.
3. Give no notification a badge.
4. Clear it when a desktop app quits.
5. Update the matrix above and [`capabilities.md`](capabilities.md).
