// Drives the running macOS client through the Accessibility API + CGEvent, so a macOS flow can be
// exercised end-to-end (click a row, type into a field, dismiss a dialog) rather than only booted
// into a state by a MAILCAL_* hook. scripts/dev/control.sh macos is the entry point; see there.
//
// Two halves, and the read half is the valuable one:
//
//   * `dump` / `find` read the Accessibility tree. That tree is the ASSERTION ORACLE; it says what
//     the app actually shows, in a form you can grep. An `AXSheet` node means a dialog is up; a text
//     field's value proves a draft survived. Prefer it to eyeballing a screenshot: it's exact, and
//     it doesn't cost a vision round-trip.
//   * `tap` / `text` / `key` post CGEvents. Coordinates come from `find`, so a flow doesn't hardcode
//     pixels and doesn't break when the layout shifts.
//
// Requires ACCESSIBILITY PERMISSION for whichever app hosts the terminal (Terminal, iTerm, VS Code,
// Claude Code): System Settings -> Privacy & Security -> Accessibility. Without it both halves fail
//; AXUIElementCopyAttributeValue returns nothing and synthetic clicks are swallowed SILENTLY, which
// looks exactly like "the app ignored the click". `require(trusted)` below turns that into a real
// error instead of a confusing no-op; exit code 3 means "not trusted".
//
// Known limit: SwiftUI `.swipeActions` do NOT fire from synthetic events (any scroll phase); that
// gesture needs a real trackpad. Everything else here (rows, buttons, fields, sheets) drives fine.
//
// Exit codes: 0 ok · 1 usage/not-found · 2 app not running · 3 no Accessibility permission.

import Cocoa

// The app registers as "AllodiaMail" (the target name) as a process, but its windows carry
// CFBundleDisplayName; accept either, and fall back to the bundle id. "Allodia Mail" is the
// display name this app carried before the product's full name went into the Info.plist; it
// stays in the list so an installed older copy is still reachable from these scripts.
let appNames = ["AllodiaMail", "Allodia Mail & Calendar", "Allodia Mail"]
let bundleFragment = "mailcal"

func fail(_ message: String, _ code: Int32) -> Never {
    FileHandle.standardError.write(Data("error: \(message)\n".utf8))
    exit(code)
}

/// The **debug build only**; never a copy installed in `/Applications`.
///
/// A developer running this repo very often has their own release build open on their real
/// accounts at the same time, and both processes answer to the same name and bundle id. Picking
/// `runningApplications.first` therefore chose between them by list order, which is to say by
/// luck: a `tap` meant for the harness could land in a real mailbox. That is not a stray
/// keystroke; the reading view now carries Accept / Decline, and a tap there emails a real
/// organiser. The Android skill already states this rule for a physical phone
/// (`docs/debugging.md`); macOS has exactly the same exposure and had no guard at all.
///
/// So the match is narrowed to a bundle inside a **repo checkout**, and a lone installed copy is
/// refused with an explanation rather than driven. There is no override: an escape hatch here
/// would be a footgun whose only use is the case this exists to prevent.
func appElement() -> AXUIElement {
    let candidates = NSWorkspace.shared.runningApplications.filter {
        appNames.contains($0.localizedName ?? "")
            || ($0.bundleIdentifier?.contains(bundleFragment) ?? false)
    }
    guard !candidates.isEmpty else {
        fail("the macOS client is not running: boot it first with scripts/dev/boot.sh macos", 2)
    }
    // The Debug product of a checkout, and nothing else. The process name, the bundle id and
    // the window title are identical across every copy, so the path is the only thing that tells
    // them apart; and it must be the *Debug* path, not merely a checkout one: a Release build
    // sits under the same `clients/apple/build/DerivedData/` tree and is exactly what a developer
    // runs on their real accounts to try a branch out. Matching the tree alone picked between the
    // two by list order, which is the coin-flip this function exists to remove.
    let development = candidates.filter {
        ($0.bundleURL?.path ?? "").contains("/clients/apple/build/DerivedData/Build/Products/Debug/")
    }
    guard let app = development.first else {
        fail(
            """
            no *debug* Allodia Mail & Calendar is running. Only \
            \(candidates.compactMap { $0.bundleURL?.path }.joined(separator: ", ")), \
            which is a build on the developer's own accounts (an installed copy, or a Release \
            build of a branch). Refusing to drive it: a synthetic tap in the reading view can \
            archive real mail or answer a real meeting invitation. Boot the debug build first with \
            scripts/dev/boot.sh macos --account stalwart
            """, 2)
    }
    app.activate()
    usleep(300_000)  // let the window come forward before we read or click it
    return AXUIElementCreateApplication(app.processIdentifier)
}

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}

func children(_ element: AXUIElement) -> [AXUIElement] {
    attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
}

func string(_ element: AXUIElement, _ name: String) -> String {
    attribute(element, name) as? String ?? ""
}

func center(_ element: AXUIElement) -> CGPoint? {
    guard let position = attribute(element, kAXPositionAttribute),
          let size = attribute(element, kAXSizeAttribute)
    else { return nil }
    var origin = CGPoint.zero
    var extent = CGSize.zero
    AXValueGetValue(position as! AXValue, .cgPoint, &origin)
    AXValueGetValue(size as! AXValue, .cgSize, &extent)
    return CGPoint(x: origin.x + extent.width / 2, y: origin.y + extent.height / 2)
}

/// A node's identifying text: SwiftUI spreads it across title, value and description.
func label(_ element: AXUIElement) -> String {
    [
        string(element, kAXTitleAttribute),
        string(element, kAXValueAttribute),
        string(element, kAXDescriptionAttribute),
    ]
    .filter { !$0.isEmpty }
    .joined(separator: " | ")
}

/// Roles worth printing even when they carry no text; an empty text field still needs coordinates,
/// and an AXSheet with no label is exactly the "a dialog is up" signal a flow asserts on.
let structuralRoles: Set<String> = [
    "AXButton", "AXRow", "AXTextField", "AXTextArea", "AXWebArea", "AXSheet", "AXCheckBox",
    "AXPopUpButton", "AXRadioButton",
]

func walk(_ element: AXUIElement, depth: Int = 0, visit: (AXUIElement, Int) -> Void) {
    if depth > 16 { return }  // SwiftUI nests deeply; this is well past any real control
    visit(element, depth)
    for child in children(element) { walk(child, depth: depth + 1, visit: visit) }
}

func windows(_ app: AXUIElement) -> [AXUIElement] {
    children(app).filter { string($0, kAXRoleAttribute) == "AXWindow" }
}

func dump(_ app: AXUIElement) {
    for window in windows(app) {
        walk(window) { element, depth in
            let role = string(element, kAXRoleAttribute)
            let text = label(element)
            guard !text.isEmpty || structuralRoles.contains(role) else { return }
            let point = center(element).map { "[\(Int($0.x)),\(Int($0.y))]" } ?? "[-,-]"
            let indent = String(repeating: "  ", count: depth)
            print("\(indent)\(role) \(point) \(text.prefix(100))")
        }
    }
}

/// Prints "<x> <y>" for the node whose text best matches `needle`, so it composes straight into tap:
///   scripts/dev/control.sh macos tap $(scripts/dev/control.sh macos find "Reply")
///
/// Ranked, not first-or-last-wins: a plain substring search for "Reply" also hits "Reply all" and
/// "noreply@example.com", and silently tapping the wrong one is the kind of bug that invalidates a
/// whole verification run. An exact label wins, then the shortest label containing it. `--all` lists
/// every hit in tree order when a flow genuinely needs to disambiguate by position.
func find(_ app: AXUIElement, _ needle: String, all: Bool) {
    var hits: [(text: String, point: CGPoint)] = []
    for window in windows(app) {
        walk(window) { element, _ in
            let text = label(element)
            guard text.lowercased().contains(needle.lowercased()), let point = center(element)
            else { return }
            hits.append((text, point))
        }
    }
    guard !hits.isEmpty else { fail("no element matching '\(needle)'", 1) }

    if all {
        for hit in hits { print("\(Int(hit.point.x)) \(Int(hit.point.y))  \(hit.text.prefix(80))") }
        return
    }
    let wanted = needle.lowercased()
    let best = hits.min { lhs, rhs in
        let lhsExact = lhs.text.lowercased() == wanted
        let rhsExact = rhs.text.lowercased() == wanted
        if lhsExact != rhsExact { return lhsExact }
        return lhs.text.count < rhs.text.count
    }!
    print("\(Int(best.point.x)) \(Int(best.point.y))")
}

/// Presses the element whose label matches `needle` through the Accessibility API's own
/// `AXPress` action; no coordinates, no cursor, no focus.
///
/// The counterpart of `control.sh linux activate`, and here for the same reason it exists
/// there: a synthetic click is a *pixel* event, so it lands wherever that point happens to be.
/// If the app's window is on another Space, behind another window, or has moved a row since the
/// dump was read, the click goes somewhere else entirely; and, being a real click, it does
/// whatever is under it. `AXPress` names the button instead, so it either presses that button or
/// fails saying it could not find it. Prefer it for anything that acts; keep `tap` for the
/// gestures AX cannot express.
func press(_ app: AXUIElement, _ needle: String) {
    var matches: [(text: String, element: AXUIElement)] = []
    for window in windows(app) {
        walk(window) { element, _ in
            let text = label(element)
            guard text.lowercased().contains(needle.lowercased()) else { return }
            guard let actions = actionNames(element), actions.contains(kAXPressAction as String)
            else { return }
            matches.append((text, element))
        }
    }
    guard !matches.isEmpty else {
        fail("no pressable element matching '\(needle)'", 1)
    }
    // Same tie-break as `find`: an exact label beats a longer one that merely contains it, so
    // "Accept" cannot resolve to "Accept and reply".
    let wanted = needle.lowercased()
    let best = matches.min { lhs, rhs in
        let lhsExact = lhs.text.lowercased() == wanted
        let rhsExact = rhs.text.lowercased() == wanted
        if lhsExact != rhsExact { return lhsExact }
        return lhs.text.count < rhs.text.count
    }!
    let status = AXUIElementPerformAction(best.element, kAXPressAction as CFString)
    guard status == .success else {
        fail("AXPress on '\(best.text)' failed (\(status.rawValue))", 1)
    }
    print("pressed \(best.text)")
}

/// The actions an element advertises, or `nil` if it advertises none (a static label).
func actionNames(_ element: AXUIElement) -> [String]? {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success else { return nil }
    return names as? [String]
}

func tap(_ point: CGPoint) {
    CGEvent(
        mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left
    )?.post(tap: .cghidEventTap)
    usleep(120_000)
    CGEvent(
        mouseEventSource: nil, mouseType: .leftMouseDown, mouseCursorPosition: point,
        mouseButton: .left
    )?.post(tap: .cghidEventTap)
    usleep(60_000)
    CGEvent(
        mouseEventSource: nil, mouseType: .leftMouseUp, mouseCursorPosition: point, mouseButton: .left
    )?.post(tap: .cghidEventTap)
}

/// Presses at `from`, moves to `to` in steps, and releases; a real mouse drag as far as AppKit and
/// SwiftUI are concerned, because it *is* one: the same `CGEvent` stream a hand produces.
///
/// The intermediate `leftMouseDragged` events are the point, and the reason this is not a `tap` with
/// a different end coordinate: SwiftUI's `DragGesture` reports `onChanged` per event, and a drag
/// delivered as one jump exercises none of the tracking a real one does. The pauses are there for
/// the same reason a real hand is slow; a gesture with no time in it cannot cross a minimum-distance
/// threshold or a long-press window.
func drag(from: CGPoint, to: CGPoint, steps: Int, holdMicroseconds: UInt32) {
    CGEvent(
        mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: from, mouseButton: .left
    )?.post(tap: .cghidEventTap)
    usleep(120_000)
    CGEvent(
        mouseEventSource: nil, mouseType: .leftMouseDown, mouseCursorPosition: from,
        mouseButton: .left
    )?.post(tap: .cghidEventTap)
    usleep(holdMicroseconds)
    for step in 1...max(steps, 1) {
        let progress = CGFloat(step) / CGFloat(max(steps, 1))
        let point = CGPoint(
            x: from.x + (to.x - from.x) * progress,
            y: from.y + (to.y - from.y) * progress
        )
        CGEvent(
            mouseEventSource: nil, mouseType: .leftMouseDragged, mouseCursorPosition: point,
            mouseButton: .left
        )?.post(tap: .cghidEventTap)
        usleep(20_000)
    }
    usleep(80_000)
    CGEvent(
        mouseEventSource: nil, mouseType: .leftMouseUp, mouseCursorPosition: to, mouseButton: .left
    )?.post(tap: .cghidEventTap)
}

/// Types Unicode directly, so it needs no keycode table and no US-layout assumption.
func type(_ text: String) {
    for character in text.utf16 {
        var unit = character
        for isDown in [true, false] {
            let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: isDown)
            event?.keyboardSetUnicodeString(stringLength: 1, unicodeString: &unit)
            event?.post(tap: .cghidEventTap)
            usleep(12_000)
        }
    }
}

let keyCodes: [String: CGKeyCode] = [
    "return": 36, "enter": 36, "tab": 48, "space": 49, "delete": 51, "escape": 53, "esc": 53,
    "left": 123, "right": 124, "down": 125, "up": 126,
]

func key(_ name: String) {
    guard let code = keyCodes[name.lowercased()] else {
        fail("unknown key '\(name)' (\(keyCodes.keys.sorted().joined(separator: "|")))", 1)
    }
    for isDown in [true, false] {
        CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: isDown)?
            .post(tap: .cghidEventTap)
        usleep(30_000)
    }
}

// --- entry point -------------------------------------------------------------------------------

guard AXIsProcessTrusted() else {
    fail(
        """
        no Accessibility permission, so the app cannot be read or clicked.
        Grant it to the app hosting this terminal (Terminal / iTerm / VS Code / Claude Code):
          System Settings -> Privacy & Security -> Accessibility
        Without it, AX reads come back empty and synthetic clicks are silently swallowed.
        """, 3)
}

let arguments = Array(CommandLine.arguments.dropFirst())
guard let action = arguments.first else {
    fail("usage: macos-ax.swift <dump|find|press|tap|drag|text|key> [args...]", 1)
}
let app = appElement()

switch action {
case "dump":
    dump(app)
case "find":
    guard arguments.count >= 2 else { fail("find <text> [--all]", 1) }
    find(app, arguments[1], all: arguments.contains("--all"))
case "press":
    guard arguments.count >= 2 else { fail("press <label>", 1) }
    press(app, arguments[1])
case "tap":
    guard arguments.count >= 3, let x = Double(arguments[1]), let y = Double(arguments[2]) else {
        fail("tap <x> <y>", 1)
    }
    tap(CGPoint(x: x, y: y))
case "drag":
    guard
        arguments.count >= 5,
        let x1 = Double(arguments[1]), let y1 = Double(arguments[2]),
        let x2 = Double(arguments[3]), let y2 = Double(arguments[4])
    else {
        fail("drag <x1> <y1> <x2> <y2> [hold-ms]", 1)
    }
    // A press held before the first movement; the only way to reach anything gated on a long
    // press, and harmless to a gesture that is not.
    let holdMs = arguments.count >= 6 ? (Double(arguments[5]) ?? 0) : 0
    drag(
        from: CGPoint(x: x1, y: y1), to: CGPoint(x: x2, y: y2),
        steps: 12, holdMicroseconds: UInt32(holdMs * 1000)
    )
case "text":
    guard arguments.count >= 2 else { fail("text <string>", 1) }
    type(arguments[1])
case "key":
    guard arguments.count >= 2 else { fail("key <return|escape|tab|delete|up|down|left|right>", 1) }
    key(arguments[1])
default:
    fail("unknown action '\(action)' (dump|find|press|tap|drag|text|key)", 1)
}
