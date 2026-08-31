// Prints the CGWindowID of the first on-screen, normal-layer window owned by the named app, so
// `screencapture -l <id>` can grab the app's window alone rather than the whole screen (what
// scripts/dev/screenshot.sh does). A store screenshot must not show the rest of the desktop.
//
// Run with `xcrun swift scripts/dev/macos-window-id.swift "Allodia Mail & Calendar"`. Swift
// rather than a
// shell one-liner because CGWindowListCopyWindowInfo is the only API that maps an app name to a
// window id without Accessibility permission; which `osascript`/System Events would require.
//
// Pass `--activate` to bring the app forward first and refuse unless it gets there. `screencapture`
// photographs a window whether or not it is *key*, and an inactive macOS window draws itself
// differently: grey traffic lights, a grey default button, a grey selected row. So a capture taken
// while anything else holds focus comes back looking like a disabled app; and nothing downstream
// can tell, because the file is present, the right size, and not blank. That shipped a full docs
// set once; the same code path feeds the store screenshots.
//
// Exit codes: 0 printed an id · 1 the window list was unavailable · 2 the app has no such window ·
// 3 the window exists but is not on the active Space · 4 the app would not come to the front.
//
// 3 is split out from 2 because the two need opposite responses and look identical from the shell.
// `.optionOnScreenOnly` means *the Space you are looking at*: an app whose windows live on another
// desktop is running, healthy, and completely invisible here. That is not hypothetical; a second
// instance of this bundle id launched while the developer's own Allodia Mail is open on another
// Space joins it there, so every capture in a run fails while the app is demonstrably fine. Told
// only "no on-screen window", the next person reads the phrase after it ("did it crash?") and goes
// looking through a log that says the scene appeared.

import AppKit
import CoreGraphics
import Foundation

let arguments = CommandLine.arguments.dropFirst()
let shouldActivate = arguments.contains("--activate")
// The default is CFBundleDisplayName from clients/apple/project.yml; callers pass
// scripts/dev/lib.sh's APPLE_APP_NAME, which is the same string kept in one place.
let owner = arguments.first(where: { !$0.hasPrefix("--") }) ?? "Allodia Mail & Calendar"

/// Brings `pid` forward and waits for the system to agree, because activation is asynchronous:
/// returning the instant `activate()` is called would photograph the window one frame before it
/// becomes key, which looks exactly like the bug this is here to prevent.
func bringToFront(pid: pid_t) -> Bool {
    guard let app = NSRunningApplication(processIdentifier: pid) else { return false }
    if NSWorkspace.shared.frontmostApplication?.processIdentifier == pid { return true }
    app.activate()
    for _ in 0..<40 {
        if NSWorkspace.shared.frontmostApplication?.processIdentifier == pid { return true }
        Thread.sleep(forTimeInterval: 0.05)
    }
    return false
}

/// The app's normal (layer 0) windows in one window list.
func normalWindows(_ options: CGWindowListOption) -> [[String: Any]]? {
    guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]]
    else { return nil }
    return windows.filter {
        $0[kCGWindowOwnerName as String] as? String == owner
            // Layer 0 is a normal window; menu bars, popovers and the Dock sit on other layers.
            && $0[kCGWindowLayer as String] as? Int == 0
    }
}

guard let onScreen = normalWindows([.optionOnScreenOnly, .excludeDesktopElements]) else {
    FileHandle.standardError.write(Data("error: could not read the window list\n".utf8))
    exit(1)
}

if let window = onScreen.first(where: { $0[kCGWindowNumber as String] as? Int != nil }) {
    let id = window[kCGWindowNumber as String] as? Int ?? 0
    if shouldActivate, let pid = window[kCGWindowOwnerPID as String] as? pid_t, !bringToFront(pid: pid)
    {
        FileHandle.standardError.write(
            Data(
                """
                error: '\(owner)' would not come to the front, so it would be photographed \
                inactive: grey buttons, grey selection, and nothing downstream able to tell. \
                Something else is holding focus (a modal dialog, a full-screen app, or a login \
                window). Clear it and re-run.

                """.utf8))
        exit(4)
    }
    print(id)
    exit(0)
}

// Nothing on this Space. Before blaming the app, ask whether it has a window at all.
let anywhere = normalWindows([.optionAll, .excludeDesktopElements]) ?? []
if !anywhere.isEmpty {
    FileHandle.standardError.write(
        Data(
            """
            error: '\(owner)' has \(anywhere.count) window(s), none on the Space you are looking \
            at. It did not crash. Switch to the desktop showing it, or quit the other instance of \
            the same app and re-run.

            """.utf8))
    exit(3)
}

FileHandle.standardError.write(Data("error: no window at all owned by '\(owner)'\n".utf8))
exit(2)
