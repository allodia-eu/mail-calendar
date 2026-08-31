---
name: debug-app
description: Boot Allodia Mail & Calendar on a platform (macos, iphone, ipad, android, linux; windows is Windows-only) against the local Stalwart harness, then observe it (background log tail, screenshots) and drive it with deterministic MAILCAL_* launch hooks or semantic input. Use to reproduce or verify UI/behaviour per platform without a personal account.
---

# debug-app: boot, observe, and control a client

An end-to-end debugging loop for the native clients. Default to the **local Stalwart harness** (see
the **mail-harness** skill), so most bugs are reproducible without personal mail. Use
`--account personal` only when the user explicitly asks for their stored accounts, for example
because a specific real message/provider behaviour triggers the bug. Scripts live in `scripts/dev/`
and are human-runnable too.

## 0. Prerequisite

Bring the harness up once: `scripts/dev/harness.sh up` (or `--bulk` for a fuller mailbox).
Host support: macOS drives `macos` / `iphone` / `ipad` / `android`; `windows` builds+runs only on
a Windows host; `linux` builds+runs on a Linux host. Android needs the SDK/NDK and a running
emulator or attached device (`adb devices`). Linux needs the package bundle in
`clients/linux/README.md`, including `/usr/bin/python3` + `python3-pyatspi` for control, **and the
GNOME SDK runtime** from that same page: `build-and-run.sh` (so `boot.sh linux`) and
`test-linux-ui.sh` build and run the client inside it, against the toolkit the Flatpak ships;
`build-and-run.sh --host` is the runtime-free loop against the distro's GTK.

**`idb` is required for `control.sh iphone|ipad`**, both for reading the simulator's accessibility tree
(`ui-dump`, `find`) and driving it (`press`, `tap`, `text`). The `MAILCAL_*` launch hooks in §4 need
nothing extra and remain the cheapest way to reach a known *state*; `idb` is how you then assert on
it. Install: `brew tap facebook/fb && brew install idb-companion && pip3 install fb-idb`.
The `idb` CLI lands in `~/Library/Python/<ver>/bin` (e.g. `~/Library/Python/3.9/bin/idb`), which is
**not on `PATH` by default**: `control.sh` will report it missing until you add that directory.

## 1. Boot

```
scripts/dev/boot.sh <macos|iphone|ipad|android|windows|linux> [--account stalwart|stalwart-imap|personal|demo] [-- <extra args>]
```

- `--account stalwart` (default): the seeded harness account `alice@test.local`, over JMAP.
- `--account stalwart-imap`: the same account over IMAP, **plus SMTP submission and a CalDAV
  calendar**. Slower to connect, but it is the only mode with **mail actions**
  (read/flag/archive/delete/move) and **IDLE push**; reach for it whenever the bug involves acting
  on a message, or mail arriving into an already-rendered list (pair it with `harness.sh deliver`).
  It is also the only mode shaped like an IMAP+CalDAV provider (mail in a mailbox beside a calendar
  on a *different* server), which is what meeting invitations break on
  ([`../../../docs/invitations.md`](../../../docs/invitations.md)): with no CalDAV there is nothing
  to answer *on* and with no SMTP no reply can be sent, so the invitation card would correctly report
  that the account cannot answer and the code under test would never be reached. Every platform
  supports it.
- `--account personal`: the developer's real stored accounts. Use only on explicit request, and
  keep log/screenshot handling privacy-aware.
- `--account demo`: the in-memory demo provider (Apple and Linux).
- **`windows`** builds + runs the WinUI client (via PowerShell) and works **only on a Windows host**;
  `demo` isn't wired there.
- Extra args after `--` pass to the client's build-and-run script (e.g. `-- --simulator "iPhone 16"`,
  `-- --no-core` to skip rebuilding the Rust core; on Windows, `-- -Arch x64` / `-- -NoRun`).

The switch works by injecting a canned account config at boot (bypassing the setup UI), so it targets
the harness on every platform even where the setup form has no JMAP tab. It is **debug-build only**
and never present in a release binary, and so is the trust of the harness's self-signed IMAP cert,
which `boot.sh` delivers to the core for `stalwart-imap`.

## 2. Read logs in the background

Start the log tail as a background job **before** or right after boot, so records stream while you
drive the app:

```
scripts/dev/logs.sh <platform>            # follow (run in the background)
scripts/dev/logs.sh <platform> --dump     # print the current log once
```

Paths (from `docs/logging.md`): macOS `~/.local/share/mailcal/mailcal.log`; iOS/iPad the simulator
container's `.../Library/Application Support/mailcal/mailcal.log` (resolved for you); Android the
Logcat `Mailcal` tag live, or the on-device file with `--dump`; Windows
`%LOCALAPPDATA%\Allodia\MailCalendar\logs\app.log` (followed for you). The core logs only
counts/ids/durations/events, never content/addresses/credentials, so it's safe to read.
When personal accounts are involved, prefer current-session slices over full historical dumps; older
logs can predate the privacy-safe path or carry account identifiers from prior debugging.

## 3. See it: screenshots

```
scripts/dev/screenshot.sh <platform> [out.png]   # prints the saved path
```

macOS captures the screen; simulators use `simctl io`; Android uses `screencap`; Linux captures the
live GTK window through GNOME on a desktop or raw X pixels only in the private Xvfb test session.

⚠️ **On Linux a window capture cannot see a popover**: an autosuggest list, a menu, a dropdown and
a tooltip each live in their own surface, so the picture shows the state *before* the thing you are
debugging and a working feature reads as a broken one. Use `control.sh linux ui-dump` (which sees
it, and is the assertion oracle) or a full-screen `gnome-screenshot` with no `--window`.
`docs/debugging.md` §4 has the details.

For the **store screenshot set** (the message list, a reply in progress, Settings, Add account, in
English and Dutch), use `scripts/dev/showcase.sh <platform>` instead. It boots the seeded in-memory
showcase dataset (`MAILCAL_SHOWCASE`, no real account, no network, background sync off), relaunches
once per screen, and captures at store-valid sizes into `showcase-screenshots/`. See
`docs/debugging.md` §4b. Don't hand-drive these with taps; the launch flags are deterministic.

## 4. Control it

**Prefer the deterministic launch hooks** (layout-independent, reliable): set a `MAILCAL_*` env
var and boot, which drops the app into a known state; then screenshot. Examples:

```
MAILCAL_OPEN_FIRST=1 scripts/dev/boot.sh macos     # open the first message
MAILCAL_CALENDAR=1   scripts/dev/boot.sh iphone    # start on the calendar
```

For direct input where a stable CLI exists:

```
scripts/dev/control.sh android tap <x> <y> | text "<s>" | key back|enter | swipe ... | ui-dump
scripts/dev/control.sh iphone ui-dump | find "<label>" | press "<label>" | probe <x> <y> | tap <x> <y> | text "<s>"
scripts/dev/control.sh macos  tap <x> <y> | text "<s>" | key return|escape|... | find "<label>" | ui-dump
scripts/dev/control.sh linux  activate "<accessible name>" | find "<accessible name>" | set-text "<accessible name>" "<value>" | ui-dump
```

`ui-dump` prints the accessibility tree so you can locate semantic nodes (and coordinates where a
platform's adapter needs them).

**On Linux, semantic AT-SPI actions are the contract.** `activate` exact-matches the accessible
name, requires a sensitive node, and invokes only an exposed `activate` / `click` / `press` / `open`
action; `set-text` goes through AT-SPI's editable-text interface. Neither has a coordinate fallback.
GTK list rows do not consistently expose an action, so use the debug-only exact subject hook to
establish reading state:

```
MAILCAL_OPEN_SUBJECT="HTML message with a remote image" scripts/dev/boot.sh linux
scripts/dev/control.sh linux activate "Load images"
```

For the complete Linux proof, prefer `scripts/dev/test-linux-ui.sh --start-harness`. It owns a
private Xvfb + D-Bus + AT-SPI session and asserts, against the seeded harness: blocked-image →
opt-in → Reply → Send, search (narrowing, how far back, the scope filter, clearing it), the calendar
agenda plus create → detail → edit → delete, three meeting-invitation fixtures, contacts, recipient
autosuggest, and signatures. It preserves screenshots, tree, and logs under
`target/ui-test-artifacts/linux/`. It drives **no mail action**: verify archive/trash/spam by hand.

**On macOS, drive by label, never by pixel.** `find` resolves a label to coordinates that pipe
straight into `tap`, so a flow survives a layout change:

```
scripts/dev/control.sh macos tap $(scripts/dev/control.sh macos find "Reply")
scripts/dev/control.sh macos text "hello"
scripts/dev/control.sh macos find "Reply" --all     # every hit, when a label is ambiguous
```

**On iOS/iPadOS, drive by label too, never screenshot-and-guess.** `describe-all` frames and
`idb ui tap` are both in **points**, so a label resolves straight into a tap with no scaling and no
Simulator-window arithmetic. `ui-dump` prints the same `Role [x,y] Label` lines the macOS dump does,
so a grep ports between them. Reaching for a screenshot to locate a control is the old, expensive
loop; a dump and a `press` replace it:

```
scripts/dev/control.sh iphone ui-dump | grep AXButton
scripts/dev/control.sh iphone press "Accept this invitation"     # find + tap in one call
scripts/dev/control.sh iphone find "Reply" --all                 # disambiguate: "Reply" vs "Reply all"
```

`control.sh` resolves the booted simulator and starts its `idb_companion` for you, and always passes
`--udid`. **Do not hand-roll a bare `idb` call**: once a second simulator has ever been used it dies
with *"No udid provided and there are multiple companions"*, and `idb connect` alone leaves a stale
socket that refuses connections.

**Known gap: the navigation bar does not enumerate.** The top bar arrives as one unlabelled
`AXGroup [220,89]` with its items omitted, so `find`/`press` cannot reach Compose, More, Send or
Cancel. They *are* in the accessibility tree (VoiceOver reaches them; `probe` returns a labelled
`AXButton 'Compose'` for the same pixel) and idb stops identically in Apple's own Settings app, so
**never report a missing toolbar item as an app bug.** Identify it by pixel, then tap:

```
scripts/dev/control.sh iphone probe 400 89      # -> AXButton [395,84] Compose
scripts/dev/control.sh iphone tap 400 89
```

**Assert VoiceOver reachability, not pixels: this is what a screenshot cannot do.** A container
that sets `.accessibilityLabel` on `.accessibilityElement(children: .contain)` stays expanded on
macOS but **collapses into one node on iOS**, so every control inside becomes unreachable while
still looking perfect in a picture. That shipped once (the invitation card: VoiceOver read "Meeting
invitation from Bob Tester" and stopped, with no Accept/Maybe/Decline). After adding or wrapping a
container, dump **both** platforms and compare: a difference is the signal.

```
scripts/dev/control.sh iphone ui-dump | grep -cE "Organiser|When|Where"   # 0 on a collapsed card
scripts/dev/control.sh macos  ui-dump | grep -cE "Organiser|When|Where"   # macOS keeps the children
```

**On iOS/iPadOS a `Toggle` needs a *held* tap: an instant one is silently swallowed.**
`idb ui tap x y` posts touch-down and touch-up together. A SwiftUI `Toggle` is a `UISwitch`, i.e. a
`UIControl`, and a `UIControl` inside a `UIScrollView` gets `delaysContentTouches` (~150 ms), so
the scroll view is still deciding whether the gesture is a pan when the finger lifts, and the switch
never sees it. SwiftUI `Button`s are gesture-based and are *not* affected, which is the trap: buttons
respond, the switch does not, and it reads exactly like a broken control. It is not: pass a
duration.

```
idb ui tap --udid <U> --duration 0.25 <x> <y>     # a Toggle / UISwitch
idb ui tap --udid <U> <x> <y>                     # a Button is fine instantly
```

Verify against the switch's real state (`AXValue` is `0`/`1` in `idb ui describe-all`), never against
"the tap didn't error".

**Re-read the frame after every change: iOS layout moves under you.** A centred screen re-centres
when a disclosure expands, so coordinates from an earlier `describe-all` land on the wrong element
(or on nothing). Resolve the frame and tap in the *same* step.

**`ui-dump` is the assertion oracle, not just a coordinate source.** It says what the app actually
shows, in a form you can grep, so assert on it rather than eyeballing a screenshot (exact, and no
vision round-trip). An `AXSheet` node means a dialog is up; a text field's value proves a draft
survived:

```
scripts/dev/control.sh macos ui-dump | grep -iE "AXSheet|Discard draft"   # is the prompt up?
```

macOS needs **Accessibility permission for the terminal's host app** (Terminal / iTerm / VS Code /
Claude Code): System Settings → Privacy & Security → Accessibility. Without it, AX reads come back
empty and synthetic clicks are swallowed **silently**, which looks exactly like the app ignoring the
click. `control.sh macos` fails loudly with that instruction instead (exit 3); ask the user to grant
it. Known limit: SwiftUI **`.swipeActions` do not fire from synthetic events**: that gesture needs a
real trackpad, so ask the user to test a swipe by hand rather than concluding it's unsupported.

## 5. Debug the state source first

For UI mismatches, first identify which Rust/FFI snapshot field drives the copy, then trace the
`Surface` signal that should make the client re-pull it. A view can be correct while its cached
snapshot is stale; settings and connectivity bugs often live at that boundary.

## Physical iOS device (background sync + notifications)

The steps above are **simulator/emulator** based. Background delivery can't be tested there:
`BGTaskScheduler` doesn't run on a simulator and banners don't render, so for background sync +
new-mail notifications use a **real iPhone/iPad** via `scripts/dev/device.sh` (the dedicated
`ios-device-bgsync` skill has the full narrative):

```
scripts/dev/device.sh doctor          # device + Developer Mode + signing team
scripts/dev/device.sh all             # build + install + launch (add a real account in the app)
scripts/dev/device.sh bgsync          # one pass; prints the mark before/after (SEEDED vs DETECTED)
scripts/dev/logs.sh ipad --device     # pull the on-device log (or: device.sh logs)
```

For a self-serve loop on a **simulator/emulator**, pair the background trigger
(`MAILCAL_RUN_BGSYNC=1` on iOS; `adb shell am broadcast -a eu.allodia.mailcal.DEBUG_RUN_SYNC -p
eu.allodia.mailcal` on Android) with `scripts/dev/harness.sh deliver` to inject new inbound mail.

## Typical flow

1. `scripts/dev/harness.sh up`
2. Start `scripts/dev/logs.sh macos` as a background job.
3. `scripts/dev/boot.sh macos` (defaults to the harness).
4. `scripts/dev/screenshot.sh macos` and read the streamed log to confirm behaviour.
5. Reproduce a bug, fix code, rebuild with `boot.sh` again, and re-observe.

Whenever the fix is a bug/regression fix, add a deterministic regression test at the lowest shared
layer that observes the contract, usually `mailcal-app` or `mailcal-bindings`. If that is not
practical, record why and the manual verification path.
