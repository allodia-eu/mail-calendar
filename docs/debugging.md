# Debugging the clients against a local mail server

This is the developer (and AI-agent) loop for running any client, **macOS, iPhone/iPad
simulators, Android, Windows, Linux**, against a **local, seeded [Stalwart](https://stalw.art) mail
server** instead of a personal account. It means you can reproduce and fix bugs without depending
on anyone's live mailbox, and every developer gets the same deterministic dataset.

Everything here is **debug-build only**: the dev-account switch and the IMAP trust path are
compiled out of release binaries and never reach a shipped app.

The `mail-harness` and `debug-app` skills wrap `scripts/dev/*` (human-runnable) for every client,
including Linux boot, logs, screenshots, semantic AT-SPI control, and the headless acceptance path.


## Never send outside the harness

**A reply to a seeded fixture opens with an EXTERNAL address already in To.** The harness's mail is
deliberately real-looking (`news@example.com`, `ahmed.elamrani@example.eu`, …), a reply pre-fills its
recipients from the original, and the harness's outbound queue will genuinely try to reach that
domain, and keep retrying for days, filling the inbox with delay warnings. Nothing on screen says
the recipient is not local.

So: **when `MAILCAL_DEV_ACCOUNT=stalwart*`, every recipient must be `@test.local`** (`alice@`,
`bob@`). Clear the pre-filled To before sending, and check Cc: that is the one that gets left
behind, because a plain reply leaves it empty and a *reply-all* does not.

On Windows this is now a **gate, not a rule to remember**: a DEBUG build connected to a harness dev
account refuses such a send and logs which recipients were outside `test.local`
(`Services/HarnessRecipientGate.cs`). It is compiled out of release builds and inactive under
`--account personal`, which is real mail and must send wherever you say. The other clients do not
have it yet; there, the discipline is manual.

## Prerequisites

- **Docker** (for the harness).
- **macOS + Xcode** to run the Apple clients (`macos` / `iphone` / `ipad`); the iPhone/iPad run in
  the simulator, so no device is needed.
- **Android**: the SDK + NDK (`ANDROID_HOME`) and a running emulator or attached device
  (`adb devices`). Runs on macOS or Windows.
- **Windows**: the WinUI client builds and runs **only on Windows**. Run `scripts\dev` there, or
  `clients/windows/build-and-run.ps1`.
- **Linux**: install the complete Ubuntu build/capture/control bundle from
  [`clients/linux/README.md`](../clients/linux/README.md#prerequisites), then use
  `clients/linux/build-and-run.sh` or the shared `boot.sh` command below. That bundle includes GTK,
  libadwaita, WebKitGTK, `gnome-screenshot`, ImageMagick, FFmpeg, `wmctrl`, `xdotool`, X11
  inspection tools, AT-SPI inspection, and Xvfb with their exact `apt` package names. Install the
  **GNOME SDK runtime** from that same page too: `build-and-run.sh` and `test-linux-ui.sh` build and
  run the client *inside* it ([`scripts/dev/sdk.sh`](../scripts/dev/sdk.sh)), so the toolkit under
  test is the one the Flatpak links rather than whichever GTK the distribution carries. It is a
  ~2 GB one-time install neither script performs; `build-and-run.sh --host` is the runtime-free
  inner loop against the `apt` packages.
- **`idb`**: needed for `control.sh iphone|ipad`, which is both how you *read* the simulator's
  accessibility tree (`ui-dump`, `find`) and how you drive it (`press`, `tap`, `text`). Install with
  `brew tap facebook/fb && brew install idb-companion && pip3 install fb-idb`; pip puts the `idb`
  CLI under `~/Library/Python/<ver>/bin`, which is **not** on `PATH` by default. Add it, or
  `control.sh` will report `idb` as missing. Nothing else needs it: the launch hooks below (and
  `showcase.sh`) drive the app without synthetic input.

## 1. Start the local mail server

```sh
scripts/dev/harness.sh up            # start + seed; blocks until healthy
scripts/dev/harness.sh up --bulk     # also seed dozens of extra varied messages (fuller mailbox)
scripts/dev/harness.sh status        # health + host ports + accounts
scripts/dev/harness.sh reset         # wipe and re-bootstrap from empty, clients included
scripts/dev/harness.sh test          # gated JMAP live test: a fast smoke check
scripts/dev/harness.sh deliver       # SMTP-inject a fresh message (bob->alice): new inbound mail
scripts/dev/harness.sh down          # stop
```

`deliver` (optionally `--to`/`--from`/`--subject`) submits a message over the harness SMTP so a
**background-sync 'detect' pass has new mail to find**, the self-serve inbound side of the
background-sync loop (§6) on a simulator/emulator.

It seeds `alice@test.local` (password `harness-alice-pw`) with threads, an attachment, flagged/read
states, duplicate/missing Message-IDs, and calendar events; `--bulk` adds a larger volume across
dev-only folders (`Lists`, `Newsletters`, `Bulk`, `DeepThread`). Full detail:
[`../docker/stalwart/README.md`](../docker/stalwart/README.md).

## 2. Boot a client against it

```sh
scripts/dev/boot.sh macos                       # defaults to the harness (JMAP)
scripts/dev/boot.sh iphone                       # iOS simulator
scripts/dev/boot.sh ipad
scripts/dev/boot.sh android                      # connected device/emulator via adb reverse
scripts/dev/boot.sh windows                      # WinUI 3 (Windows host only; builds via PowerShell)
scripts/dev/boot.sh macos --account personal     # your real stored accounts (unchanged behavior)
scripts/dev/boot.sh linux                        # GTK4/libadwaita (Linux host only)
scripts/dev/boot.sh macos --account stalwart-imap # IMAP + SMTP + CalDAV instead of JMAP (see below)
scripts/dev/boot.sh macos --account stalwart-multi # TWO harness accounts (cross-account contacts)
scripts/dev/boot.sh windows --account stalwart-multi # the same, on WinUI
scripts/dev/boot.sh iphone -- --simulator "iPhone 16"   # args after -- pass to build-and-run
```

`boot.sh` always picks a **simulator** for iphone/ipad, even with an iPhone plugged in, because the
harness it boots against is loopback-only and nothing on a physical device can reach it. The plain
dev loop prefers the connected device instead:
[`clients/apple/Scripts/build-and-run.sh`](../clients/apple/Scripts/build-and-run.sh) `--iphone`,
which runs against a real account added in the app.

The switch is the `MAILCAL_DEV_ACCOUNT` environment variable (`stalwart` | `stalwart-multi` |
`stalwart-imap` | `personal` | `demo` | `first-run`), which each client's build-and-run script
honours. Every `stalwart` mode injects a canned account config at boot, **bypassing the setup UI**,
so it works the same on every platform with no form to fill in. Each harness mode uses its **own
engine store** (`mailcal-dev` / `dev` for JMAP, `mailcal-dev-multi` / `dev-multi` for the
two-account JMAP boot, `mailcal-dev-imap` / `dev-imap` for IMAP), so its test data never mixes with
your real accounts, nor one mode's with the other's.

`first-run` (Apple and Windows) is the odd one: it injects **nothing**, and its namespace
(`mailcal-dev-first-run` / `dev-first-run`) is isolated for the opposite reason to the others:
so the screens somebody sees **once**, the analytics consent and then the first-account screen
([`onboarding.md`](onboarding.md)), can be seen again. Every other mode either injects an account
or reads the namespace you are already using, so neither can show a first run without emptying
something you wanted. Delete the directory to get the first run back; anything added through the
form persists there until you do.

> ⚠️ **Relaunching a simulator build by hand silently switches it to the personal account.**
> `xcrun simctl launch <udid> eu.allodia.mailcal` passes **none** of your shell's environment to the
> app, so `MAILCAL_DEV_ACCOUNT` is simply absent and the client falls back to the developer's stored
> accounts (real mail, on screen, in whatever you were about to drive). Nothing warns you; the app
> just opens on a different mailbox. `simctl` reads the child's environment from `SIMCTL_CHILD_`-
> prefixed variables instead:
>
> ```sh
> SIMCTL_CHILD_MAILCAL_DEV_ACCOUNT=stalwart-multi xcrun simctl launch <udid> eu.allodia.mailcal
> ```
>
> Reach for this only when you need a **relaunch without a rebuild**: testing what survives a
> restart, which is the one thing `boot.sh` cannot show you, because it rebuilds and relaunches as
> one step. For anything else use `boot.sh`, which sets the variable itself.

`stalwart-imap` carries an `[smtp]` and a `[caldav]` half beside the `[imap]` one, which makes it the
only mode shaped like a real IMAP+CalDAV provider: mail in a mailbox, calendar on a *different*
server. That is the shape meeting invitations break on ([`invitations.md`](invitations.md)): with no
CalDAV there is nothing to answer *on*, and with no SMTP no reply can be sent, so the invitation card
correctly reports that the account cannot answer and no amount of testing reaches the code under
test. It was IMAP-alone until then, on every client but Windows.

> ⚠️ **`--no-core` also skips the shared composer editor.** On Apple, `build-core.sh` is what copies
> [`clients/composer/dist/editor.html`](../clients/composer/dist/editor.html) into `MailcalUI`'s SPM resources,
> so `boot.sh macos -- --no-core` (and `build-and-run.sh --no-core`) rebuilds the app around a
> **stale** editor. It fails in the worst possible way: the build is green, the app launches, and
> you verify the *old* behaviour while believing you tested the new one. If you changed
> `editor.html`, either drop `--no-core`, or copy it across by hand and **assert it landed**:
>
> ```sh
> cp clients/composer/dist/editor.html clients/apple/Packages/MailcalKit/Sources/MailcalUI/composer/editor.html
> grep -c "<a string from your change>" \
>   clients/apple/build/DerivedData/Build/Products/Debug/AllodiaMail.app/Contents/Resources/MailcalKit_MailcalUI.bundle/Contents/Resources/composer/editor.html
> ```

`stalwart-multi` connects the harness as **two** accounts (alice + bob). It exists for contacts: the
engine merges people across accounts on a shared address, and a single-account boot cannot show
that: the seeded `shared-*.vcf` card is filed in alice's address book *and* bob's, precisely so this
mode renders it as one row badged "In 2 accounts".

On **macOS** the isolation is wider still, and it is not conditional on the harness: **every** DEBUG
build is separated from the installed app, `--account personal` included (that being the mode that
reads and writes real credentials and real mail, and so the one that most needs it). A dev build and
the installed `.dmg` otherwise share a login keychain, a home directory, and (same bundle id) a
`UserDefaults` domain. One type decides all of it: `DevNamespace` ([`DevNamespace.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/DevNamespace.swift)):

| | shipped app | `personal` / unset / `demo` | `stalwart` | `stalwart-multi` | `stalwart-imap` |
|---|---|---|---|---|---|
| Keychain service | `eu.allodia.mailcal` | `…mailcal.dev` | `…dev.stalwart` | `…dev.stalwart-multi` | `…dev.stalwart-imap` |
| Engine store (`~/.local/share/`) | `mailcal` | `mailcal-dev-personal` | `mailcal-dev` | `mailcal-dev-multi` | `mailcal-dev-imap` |
| Preferences | `UserDefaults.standard` | suite `…mailcal.dev` | suite `…dev.stalwart` | suite `…dev.stalwart-multi` | suite `…dev.stalwart-imap` |

`personal` takes the third store name rather than the base `mailcal-dev` because the JMAP harness
already owns that one: the two would silently have shared a SQLite database. `DevNamespaceTests`
pins that no two modes collide and that none resolves to production. A dev build therefore starts
with no accounts, its own mailbox store, and its own window/diagnostics preferences.

**Known gap:** SwiftUI `@SceneStorage` (scene restoration) is written into the app's saved-state
bundle by the framework and can't be redirected, so it is still shared with the installed app. The
`NSSplitView` autosave name *is* namespaced (`AppPrefs.autosaveName`). The rotating diagnostic log
stays shared on purpose, as on Windows: one file diagnoses whatever ran last.

Three things follow that are worth knowing before you go hunting:

- **`defaults write eu.allodia.mailcal …` edits a file the app never reads.** The `defaults` CLI
  resolves a bundle id through the sandbox container (`~/Library/Containers/eu.allodia.mailcal/
  Data/Library/Preferences/`) while an unsandboxed dev build writes
  `~/Library/Preferences/eu.allodia.mailcal.plist`. Nothing errors; you just watch a value you set
  have no effect. Pass the **full path** to `defaults`/`PlistBuddy`, and `killall -u $USER cfprefsd`
  after writing (and before reading with `plutil -p`): `cfprefsd` caches, so both directions can
  show you the state before your edit. The split-view frames land in the *app's own* domain
  regardless of `AppPrefs.defaults`: AppKit writes autosave under the bundle id, which is why the
  *name* is namespaced instead.
- **Seeding a pane width to test that it persists: pick a number the layout cannot produce on its
  own.** The message list is `minWidth: 420, idealWidth: 540, maxWidth: 720`, so a check seeded
  with any of those three passes whether the width was restored or reset, and a check that cannot
  fail is not a check. Seed something like 600, then read the divider back out of
  `control.sh macos ui-dump`: the reading pane's "Select a message to read." label reports its own
  centre, which pins the divider without a screenshot.
- **A macOS dev build must be signed with a persistent certificate, or macOS will re-prompt for
  Keychain access on every rebuild.** The file keychain gates access on a **partition id**, which
  for an ad-hoc binary is its `cdhash:`, new on each build, so "Always Allow" never sticks.
  `Scripts/build-and-run.sh` re-signs with an Apple Development identity to make it `teamid:`; it
  warns loudly when it can't find one. Xcode's Run button bypasses this and stays ad-hoc.
- A permissive "any application" ACL does **not** work around it: the partition check applies even
  to items written by `security add-generic-password -A`. Don't reach for it; fix the signing.

On **Windows** the isolation extends beyond the
engine store: the credential store switches to a throwaway
`dev`/`dev-multi`/`dev-imap`/`dev-first-run` namespace (`CredentialStore.UseDevNamespace`), and the
one-line **preference files** (language, window placement, pane width, the Diagnostics log level)
move into the same dev subdir (`AppPaths.PrefsDir`), so nothing a harness run persists (a resized
window, a flipped DEBUG toggle) survives into a normal launch. The rotating log deliberately stays
shared, so one file diagnoses whatever ran last. `AppPathsTests` pins that no two dev modes resolve
to the same subdir and that none resolves to the real paths: the Windows counterpart of
`DevNamespaceTests`. For `first-run` the collision rule is what makes the mode work at all: an
account left behind by another mode is an account, and the screen would never come up.

The Linux client boots every mode. `stalwart`, `stalwart-multi` and `stalwart-imap` isolate their
engine stores under `$XDG_DATA_HOME/mailcal/dev`, `dev-multi` and `dev-imap`; `demo` is in-memory;
`--account personal` opens the developer's stored accounts through the Secret Service store
([`secrets.rs`](../clients/linux/src/secrets.rs)) the client's own setup flow writes to. An
**unrecognised** `MAILCAL_DEV_ACCOUNT` is refused: a message naming the fixtures it does boot, and
exit 2 before the window exists, never a fall-through to those stored accounts, which to whoever
asked for a harness looks exactly like a harness that seeded nothing.

### JMAP (default) vs IMAP fidelity

- **`--account stalwart` (JMAP, default)** connects over the harness's HTTP JMAP. It covers
  read / render / sync / **send** / calendar-read, and works on **all** platforms. It does **not**
  cover mail *actions* (mark-read/flag, archive, delete, move) or push, because the engine's JMAP
  adapter doesn't implement them yet (see [`jmap.md`](jmap.md) "Known gaps").
- **`--account stalwart-imap` (IMAP)** connects over the harness's implicit-TLS IMAP, which
  exercises the **full mail-action surface + IDLE push**. Supported on **every** platform: macOS,
  the iOS/iPad simulators, Windows, Linux, and the Android emulator. It needs the dev-harness custom-root
  path to trust the harness's self-signed cert (compiled into the Apple, Windows, and Linux debug cores
  via `debug_assertions`, and into Android via the `dev-harness` Cargo feature), plus that
  cert delivered to the core:

  | Platform | How the cert reaches the core |
  |---|---|
  | macOS / iOS | `MAILCAL_EXTRA_CA` names a host path the app reads directly |
  | Windows | the same, converted to a Windows path (`C:\…`). The core opens it with the Win32 file APIs, which don't understand an MSYS `/c/…` path. It rides the `bash → pwsh → Start-Process` hop by plain environment inheritance, and `build-and-run.ps1` refuses to launch if the file isn't readable |
  | Android | the emulator can't read a host path, so the cert rides base64 as an intent extra; `MainActivityCore.kt` writes it into the sandbox and sets `MAILCAL_EXTRA_CA` via `Os.setenv` |
  | Linux | `MAILCAL_EXTRA_CA` names the host path directly, as on macOS |

  The Rust core folds that PEM into `TlsPolicy::roots(true, true, custom)`, so bundled and OS roots
  stay active too: it only **adds** an anchor, and never skips chain or hostname verification.
  `boot.sh` handles every platform. Send is out of scope over IMAP (the harness SMTP is plaintext;
  the core submits over implicit TLS).

  Because IDLE is a real push, a message injected with `harness.sh deliver` lands in an
  already-rendered list on its own: no Refresh, no relaunch. That is the live-arrival path real
  accounts use, and the only faithful way to reproduce list-update bugs against the harness.

### Exercising email-first autodetection against the harness

Detection (`docs/account-autodetect.md`) needs a **fresh** app (no stored account, so the
email-first setup shows). Real public domains work over the network: `x@gmail.com` → an IMAP
result card, `x@fastmail.com` → manual (Fastmail's apex advertises no JMAP), `x@outlook.com` →
the Microsoft path, and perform no login. To exercise the **JMAP** route + a real connect
against the local Stalwart harness, whose domain (`test.local`) can't be resolved publicly,
point the JMAP probe at the harness with the debug-only env var
`MAILCAL_AUTODETECT_WELL_KNOWN_BASE` (read by the core under `debug_assertions`/`dev-harness`
only; it also waives the probe's HTTPS requirement for the local plaintext server):

  - **Android**: pass it as a launch extra and forward the port:
    `adb reverse tcp:28080 tcp:28080` then
    `adb shell am start -n eu.allodia.mailcal/.MainActivity -e MAILCAL_AUTODETECT_WELL_KNOWN_BASE http://127.0.0.1:28080`;
    type `alice@test.local` → JMAP route → connect with `harness-alice-pw`.
  - **macOS**: `MAILCAL_AUTODETECT_WELL_KNOWN_BASE=http://127.0.0.1:28080` in the app's
    environment before launch.

### Making the server refuse a login that is valid

A server can reject a credential that works: Dovecot answers `[AUTHENTICATIONFAILED]` after its
two-second `auth_failure_delay` while sibling folders on the same account authenticate in the same
second. No real test server does that on demand, so
[`imap-fault-proxy.py`](../scripts/dev/imap-fault-proxy.py) stands in front of the harness, passes
every byte through, and answers only the auth command itself:

```sh
scripts/dev/harness.sh up
scripts/dev/boot.sh android --account stalwart-imap           # build + install once, on the harness
python3 -u scripts/dev/imap-fault-proxy.py --refuse-all       # or --refuse-every 5
adb reverse tcp:12993 tcp:12994                               # AFTER boot.sh, which re-points it
```

Then drive each case by **replaying the launch**, because the trust root is delivered as a launch
extra and a dial through the proxy has to start with it in place:

```sh
PEM="$(base64 < docker/stalwart/tls/fault-proxy-cert.pem | tr -d '\n')"
adb shell am force-stop eu.allodia.mailcal
adb shell am start -n eu.allodia.mailcal/.MainActivity \
  -e MAILCAL_DEV_ACCOUNT stalwart-imap -e MAILCAL_EXTRA_CA_PEM "$PEM"
```

The proxy prints a line per connection (`conn[5] login REFUSED`) to correlate against the app log.
`--refuse-every 5` lands on a **role folder** after the INBOX has authenticated, which is the mixed
dial; `--refuse-all` refuses the account's first login. To make the credential work again **without**
relaunching, restart the proxy with no flags: a plain pass-through keeps the same certificate, so
re-pointing the reverse back at the harness instead fails the next dial with `UnknownIssuer` (the app
is still trusting the proxy). One thing it cannot do: make one folder of an *already-connected*
account fail, because IMAP keeps its authenticated session (`docs/provider-oauth.md` → Known gaps).

## 3. Read logs in the background

Run the log tail as a background job so records stream while you drive the app:

```sh
scripts/dev/logs.sh macos            # follow the rotating file log
scripts/dev/logs.sh android --dump   # print the current on-device log once
scripts/dev/logs.sh windows          # follow the Windows app.log (--dump prints it once)
scripts/dev/logs.sh linux            # follow the XDG rotating file log
```

Paths and policy are the cross-platform contract in [`logging.md`](logging.md). The core logs only
counts / ids / durations / events (never mail content, addresses, or credentials), so the stream
is safe to read and attach to a support request.

### The durations in that log are a measurement: read them as one

Every main-path operation logs how long it took, one line at a time, thousands of them. Read that
way only the last line is ever seen, and the tail (the rebuild that took fifteen seconds) is
invisible. `timings.py` reduces the stream to `Operation | n | p50 | p90 | p99 | max`:

```sh
scripts/dev/timings.py                             # this machine's log + its rotations
scripts/dev/timings.py ~/Downloads/app.log         # a log a user handed over
scripts/dev/timings.py --since 2026-08-12 --top 5  # one day, and the slowest runs with timestamps
scripts/dev/timings.py --unmatched                 # timing lines no pattern claimed
```

For a device, dump first: `scripts/dev/logs.sh android --dump > app.log`.

**The table is deliberately the same shape the engine's benchmarks print.** The engine's
`cargo bench -p mailbox-fixture` measures the same operations against a synthetic mailbox of known
size (10k / 100k / 400k messages); this measures them against whatever mail the user actually has.
A change that improves one and not the other has optimised something nobody waits on, and keeping
the two tables in one shape is how that becomes visible rather than arguable.

`--unmatched` is not cosmetic. The reducer's failure mode is silence: a renamed log line stops
being counted and its row simply disappears, which reads exactly like an operation that never ran.
Run it after touching any `log::info!` that carries a duration.

## 4. See it: screenshots

```sh
scripts/dev/screenshot.sh macos [out.png]     # prints the saved path
scripts/dev/screenshot.sh iphone
scripts/dev/screenshot.sh android
scripts/dev/screenshot.sh windows             # captures the WinUI window itself (PrintWindow)
scripts/dev/screenshot.sh linux                # captures the live GTK window
```

These grab whatever is on screen right now (macOS captures the **whole screen**, and the app window may
not be frontmost). For the store screenshot set, use `showcase.sh` below instead: it captures the app
window alone, at store-valid sizes.

⚠️ **A window capture cannot see a popover, and on Linux that is most of the transient UI.** A
`GtkPopover` (the recipient autosuggest list, a menu, a dropdown, a tooltip) lives in its **own**
surface, so `gnome-screenshot --window` and `xwd -id <window>` both return the toplevel without it.
So does `xwd -root` under a compositor. The screenshot therefore shows the state *before* the thing
you are debugging, which reads exactly like a feature that does not work. It costs an afternoon
once. Three ways to see what is really there, cheapest first:

```sh
scripts/dev/control.sh linux ui-dump | grep -i "<the popover's list>"   # AT-SPI sees it
xwininfo -root -children | grep mailcal-linux                           # its own X window, sized and placed
gnome-screenshot --file /tmp/screen.png                                 # no --window: the whole screen
```

The AT-SPI dump is the one to reach for: it is also the assertion oracle, so a popover you can grep
for is a popover you can write a `test-linux-ui.sh` leg against.

### Linux semantic acceptance

The Linux screenshot adapter uses `gnome-screenshot` for the compositor-visible window on a regular
desktop. The acceptance wrapper instead owns a private, compositor-free Xvfb display, so it can use
the X backing pixels safely:

```sh
scripts/dev/screenshot.sh linux /tmp/mailcal-linux.png
scripts/dev/test-linux-ui.sh --start-harness
```

The wrapper drives one Stalwart-backed run, in order: the seeded remote-image fixture: the privacy
banner, opt-in, Reply only after the editor announces readiness, the derived recipient as a pill,
Send, the success banner; **search**: narrowing, the how-far-back line, the scope filter, and the
unsearched list back when the field is cleared; the **calendar** agenda plus create → detail → edit
→ delete; the three seeded **meeting-invitation** fixtures: the iMIP request with its three answer
buttons and day preview, the published `.ics` that must draw no card, and the one landing on an
empty day; **contacts**: the sections, two namesakes staying two rows, and search; **recipient
autosuggest** in the composer; and **signatures**: the empty state, both slot pickers, and a
signature created through the core. It saves each state, the AT-SPI tree, and logs under
`target/ui-test-artifacts/linux/<timestamp>`. It never uses a personal account, screen coordinates,
or synthetic key events. The exact Ubuntu packages and the Wayland/X11 boundary are documented in
[`clients/linux/README.md`](../clients/linux/README.md#capture-and-control-the-window).

## 4b. Showcase mode: the store screenshot set

`MAILCAL_SHOWCASE` boots the app on a **seeded in-memory dataset** (two fictional accounts, a full
mailbox, a calendar) instead of any real account. Nothing is persisted, no network is touched, and
the background sync is disabled for the run, so **no personal mail can reach a screenshot** and no
new-mail notification or permission dialog can land on top of one. It is debug-build only.

The flag doubles as the language switch, because a store listing needs a set per language and Dutch
chrome over English mail reads as broken: `MAILCAL_SHOWCASE=nl` seeds Dutch sample mail *and* pins
the chrome to Dutch for that launch. It takes **any locale the shared catalog ships** (`en` · `nl` ·
`de` · `fr` · `es` · `it` · `pt`), each with its own seed, so the mail, folder names, and calendar
read in the language of the chrome around them. `MAILCAL_SHOWCASE=1` follows the app's own language
choice. `MAILCAL_SHOWCASE_SCREEN` picks the screen: `list` (default) · `reply` · `settings` ·
`add-account` · `calendar` (the headline calendar grid, on every platform) · `invitation` (a meeting
request open in the reading view, with Accept / Maybe / Decline over a preview of the day it would
land on, [`invitations.md`](invitations.md)) · `signatures` (Settings opened on the Signatures
category). `signatures` has to be **named explicitly**: it is not in `--screen all`, because only
the Windows and Linux clients can drive to it so far.

```sh
scripts/dev/showcase.sh macos                        # 42 PNGs: 6 screens x 7 languages
scripts/dev/showcase.sh iphone --locale nl
scripts/dev/showcase.sh android --screen reply --no-build
scripts/dev/showcase.sh ipad --simulator "iPad Air 13-inch (M4)"
scripts/dev/showcase.sh android --serial emulator-5554   # a specific device, with several attached
scripts/dev/showcase.sh android-tablet-10                # Google Play's 10-inch tablet slot
scripts/dev/showcase.sh android-tablet-7 --locale nl     # …and its 7-inch slot
scripts/dev/showcase.sh windows --screen settings        # on a Windows host
scripts/dev/showcase.sh linux --locale de                # on a Linux host
```

**Linux captures the same six screens as every other client**, plus `signatures` when it is named.
Its client is the one that **refuses** an unreachable screen name (exit 2, before the window exists)
rather than falling back to the mailbox list the way the others do: a clean, correctly-sized
photograph of the inbox filed under `invitation` is the failure no later check can see.

GTK single-instances on the application id, so an installed Flatpak swallows every launch a capture
run makes and the whole set photographs *it*, with the developer's real accounts in it, if any are
set up. `flatpak kill eu.allodia.mailcal` first.

They land in `showcase-screenshots/` (git-ignored) as `<target>-<locale>-<screen>.png`, at the
sizes each store wants: macOS 2880×1800, iPhone 6.9" 1320×2868, iPad 13" 2064×2752, Android phone
1080×2400, Android tablet 1200×1920 (7-inch) and 1600×2560 (10-inch), Windows 1440×900 pt
(2880×1800 on a 200%-scale display, see the note below).

**Every Android capture names the emulator it runs on**, in the git-ignored
`scripts/dev/devices.local.sh` (copy `devices.local.sh.example`): AVD names are per-machine, so
there is nothing to commit. The two tablet targets fall back to the stock SDK profile names, but the
phone has no fallback and **refuses rather than photographing the attached device**: on a
developer's own machine that is usually their real phone, and a capture run installs and drives a
build on whatever it resolved. `--avd` overrides for one run, `--serial` skips resolution entirely.

**Google Play keeps a screenshot slot per form factor**, so `android-tablet-7` and
`android-tablet-10` are targets rather than platforms: the same APK on a different emulator. Each
names an AVD (`Small_Tablet` / `Pixel_Tablet` by default), **boots it if it is
not already running and shuts it down again afterwards**, and pins the display to portrait: Play's
recommended tablet exports are portrait, it matches the rest of the set, and a single-pane layout
reads better that way (the 10-inch list shows 10 rows portrait against 6 landscape). Because the
stock tablet AVDs are landscape-native, a rotation that silently failed would produce a valid PNG
of the right app in the wrong shape, which the blank-frame floor waves straight through, so every
capture's pixel size is **asserted** against its slot, and a mismatch deletes the file rather than
leaving a wrongly-named asset behind.

The set you actually upload is this one: the captures are **not** committed, and each store's
publisher pushes them straight out of `showcase-screenshots/<platform>/`
([`store-screenshots.md`](store-screenshots.md) has the rules and the
reasoning). Two Play-only wrinkles
live there rather than here: the Android sets share **one flat directory** with a `phone-` /
`tablet-7-` / `tablet-10-` file-name prefix (the Play Console's picker is a flat list), and the
**feature graphic** Play requires to publish a listing is *composed* from them rather than
captured, so whoever publishes regenerates it whenever the phone screenshots change.

How each platform is driven, since none of it is a pixel tap:

| | Launch flag | Chrome language | Capture |
|---|---|---|---|
| macOS | env var | `-AppleLanguages` launch arg (session-only) | `screencapture -l <CGWindowID>`: the window alone; the app sizes itself to 1440×900 pt |
| iPhone / iPad | env var via `SIMCTL_CHILD_*` | `-AppleLanguages` launch arg | `simctl io screenshot`, with a 09:41 status bar |
| Android | intent extra (no env vars) | `adb shell cmd locale set-app-locales`, reset afterwards | `adb exec-out screencap`, holding the display awake, with a 09:41 status bar |
| Windows | env var | in-memory MRT-Core override (session-only) | `PrintWindow` + `PW_RENDERFULLCONTENT`: the window alone; the app sizes itself to 1440×900 pt |

**Windows sizes in logical units, not pixels.** The app pins 1440×900 *epx* and lets the monitor
scale decide the physical frame (1440×900 at 100%, 2880×1800 at 200%), so every capture is inside
the Microsoft Store's 1366×768…3840×2160 bounds *and* the reading pane stays above its `MinWidth`.
Pinning 1920×1080 physical instead would be only 960×540 logical on a HiDPI screen, and the reading
pane would collapse. A showcase run also ignores the developer's saved window placement and pane
split, and does not persist its own: a capture pass must not rearrange your app.

**A screenshot cannot tell you it is showing fictional mail.** The `MIN_CAPTURE_BYTES` floor only
rejects a *blank* frame; a screenshot of a real, fully-populated mailbox is a perfectly plausible
300 kB PNG. A binary built before showcase mode existed ignores `MAILCAL_SHOWCASE` entirely, opens
the developer's accounts, and photographs them, silently and photogenically. So **every** capture
asserts the property positively, before the shutter.

The signal is one line, shared by all four clients: `boot::build_showcase` logs it from *inside* the
in-memory engine's constructor, and the `Logger` FFI port routes it into each client's diagnostic
log. Its presence proves the fictional engine was really built, not merely that a flag was read.
`showcase.sh` reads only the bytes **this launch** appended, so an earlier run's line can't vouch for
the current one, and it checks the seeded locale too. A failed assertion stops the client rather than
leaving a window of real mail in front of a shutter. The marker lives in `scripts/dev/lib.sh`
(`SHOWCASE_LOG_MARKER`); it is spelled in Rust, bash, and PowerShell, and
[`scripts/ci/check-showcase-flag.sh`](../scripts/ci/check-showcase-flag.sh) keeps the three in step.

Each platform's log plumbing differs (`~/.local/share` on macOS, the simulator's data container,
`run-as … files/logs/app.log` on Android), so the abort has been exercised on each, by seeding one
locale and asking for the other: it stops the client and writes no PNG. If a capture aborts with
"did NOT enter showcase mode" while the app plainly *is* in showcase mode, suspect `client_log_path`
rather than the app; the byte count in the error tells you whether the log was found at all.

Windows adds a second, earlier guard: `clients/windows/showcase.ps1` requires the built assembly to
*contain* the showcase driver before it launches anything, so a stale or Release build never opens
the real mailbox at all. (Scanning for that literal needs a raw byte search: .NET keeps string
literals as UTF-16 at arbitrary alignment, so both `Select-String -Encoding unicode` and decoding
the file as UTF-16 from offset 0 miss an odd-aligned one.)

The reply screenshot opens the composer pre-filled with sample text, which the core supplies per
locale (`showcase_reply`) and the shared editor inserts as **plain text** above the quoted original
(`docs/composer-security.md`, Gate 11), so it is neither typed by a flaky robot nor an HTML
injection path.

**`reply` is the one screen whose settle time is load-bearing, and its blank state is not blank.**
It has the furthest to go: sync, open the message, fetch its body, open the composer WebView, seed
the quote. At 11s an Android run photographed the composer mid-load: correct chrome, correct
header, correct recipient, and an empty white body with a spinner dot. That frame is **63 kB of
perfectly real app**, so the whole-frame blank floor passed it and a store asset shipped with a
loading spinner in it. The settle is now 16s, and `min_capture_bytes` gives `reply` its own floor
(120 kB, against ≥179 kB for every genuinely-loaded reply capture on every platform) so the
half-loaded case fails loudly instead of quietly. The general lesson is the recurring one: a floor
sized for *a blank screen* cannot see a **blank region** inside a populated one.

A capture pass runs for minutes, so on a **physical Android device** the screen timeout fires
part-way through and `screencap` then returns a solid black PNG while still reporting success. The
script holds the display on for the run (restoring the device's own setting afterwards), asserts
the display is `Awake` before each shutter, and rejects an implausibly small (blank) capture: an
emulator never sleeps, so this fails only on real hardware.

**The status bar is part of the screenshot.** Android's is pinned with SystemUI **demo mode**
(the counterpart of `simctl status_bar override` on the simulators), so the clock reads 09:41, the
battery is full and unplugged, and wifi is at full strength in every capture. Every element is set
explicitly: SystemUI keeps demo state across `exit`/`enter`, so anything left unnamed is whatever
the *last* run set, which is the nondeterminism the pin exists to remove. Cellular is hidden rather
than shown: true of the wifi-only tablet, and a store asset must not invent a radio the device
hasn't got. It is all undone when the run ends, including the `sysui_demo_allowed` key demo mode
needs.

**Three things reach the status bar or the frame after the pin, so all three are handled at the
last possible moment.** Each produced a store asset that looked fine until it was put beside its
sibling:

- **The first `demo enter` of a run lays the bar out wrong.** SystemUI draws the demo clock without
  its normal start padding on the very first entry and corrects it moments later, so exactly one
  screenshot per run had a subtly different status bar: always `list`, the first screen in the
  loop and the likeliest to lead a store page. `android_prepare` now enters demo mode itself, so
  the first capture is never the first `enter`.
- **The app's own new-mail notification arrives during the settle.** Clearing notifications at
  prepare time is too early: `android_prepare` runs before the build, and the build's install and
  launch post the notification, so every capture photographed an envelope beside the pinned clock.
  The clear now runs before each shutter, like the pin.
- **The composer raises the keyboard over the bottom half of the `reply` frame.** Whether the
  shutter beats it is pure timing: the English set was shot a second before it rose, a slower
  Dutch run caught it a second after, and the two locales stopped matching. It is now dismissed
  explicitly and checkably: `dumpsys input_method` reports `mInputShown`, BACK is sent **only**
  while the keyboard is actually up (with it hidden, the same key would navigate out of the
  composer), and the state is re-read to confirm. Disabling the IMEs instead does **not** work:
  Android re-enables a default as soon as the last one is gone.

**`sys.boot_completed` is not "SystemUI is ready", and the gap photographs.** Pinning the bar inside
that startup window leaves the demo wifi icon drawn *next to* the live one SystemUI is still
inflating, and the capture shows **two wifi glyphs**: a defect a size check cannot see and a tired
reviewer can. It reproduces on a cold boot and vanishes on a settled device (CPU load widens the
window rather than causing it), which is exactly how it hides: retry and it looks fine. So the pin
(a) tears demo mode down with `exit` before re-entering, which collapses an already-doubled pair
back to one, and (b) runs before **every** capture rather than once per run, which puts it far past
any boot race and keeps the last capture as firmly pinned as the first. The display rotation has the
same failure mode (set once during startup, it is undone as the device wakes), so it is re-asserted
per capture too, and the pixel-size assertion is what turns a failure there into a stopped run.

**Notification icons are the exception, and they can only be cleared, not hidden.** Demo mode's
`notifications -e visible false` is a **no-op** on current Android (verified on API 36), and
`icon_blacklist` covers system icons only. The icons belong to *other* apps regardless: the phone
emulator held `new_mail` notifications posted by `nl.allodia.mailcaldemo` (this app's own
**retired** package id), so an envelope from a build that no longer exists was photographed into
every phone screenshot, next to a Play Store icon and a Safety Centre shield.

The only lever the shell has is `cmd notification snooze`, and despite its name it is **one-way**:
`unsnooze` fails with a permission denial as the shell uid, and the snooze does not reliably repost
when it expires. So the script clears notifications **on emulators only** (`ro.boot.qemu`), where
they are throwaway, and on a **physical device** leaves them alone and says so: losing the
developer's real notifications to a screenshot run would be a bad trade. Capture store assets on an
emulator.

None of this can be hard-asserted the way the showcase-launch check is: SystemUI silently ignores
the demo broadcasts while demo mode is disallowed, and no shell command reports what the bar is
actually drawing. The script verifies the flag took; the screenshot is the rest of the evidence.

## 5. Control it

Prefer the **deterministic launch hooks** (`MAILCAL_*`), which drop the app into a known state
without pixel-tapping (reliable and layout-independent), then screenshot:

```sh
MAILCAL_OPEN_FIRST=1 scripts/dev/boot.sh macos     # open the first message
MAILCAL_CALENDAR=1   scripts/dev/boot.sh iphone    # start on the calendar
MAILCAL_OPEN_FIRST=1 scripts/dev/boot.sh windows   # same hooks on WinUI (env passes through)

# Windows only: swipe the first message row (delete | archive | star), through the same code path
# the gesture and the row's context menu use. Still useful as a *deterministic* shortcut, but see
# the correction below: it is no longer the only way to drive a gesture.
MAILCAL_SWIPE=archive scripts/dev/boot.sh windows

# Every platform: pretend the calendar server reported a verdict on the RSVP it promised to send.
# The ONLY way to reach the "the organiser wasn't told" prompt, because no harness server produces
# a reported failure (docs/invitations.md -> "Exercising it"). Reads in the CORE, so it needs no
# client support; the failure path also needs the CalDAV half of the harness.
MAILCAL_FAKE_REPLY_DELIVERY=failed:5.2 scripts/dev/boot.sh windows --account stalwart-imap

# Every platform: hold the download bar up with a staged count, so where it sits can be measured
# rather than glimpsed: a real pass is up for a fraction of a second against any local fixture.
# Reads in the CORE (it replaces the snapshot the host pulls, and nothing downstream of it), so it
# needs no client support. `<fetched>/<total>` runs determinate, a bare `<fetched>` indeterminate.
MAILCAL_FAKE_SYNC_PROGRESS=1200/3387 scripts/dev/boot.sh windows

# The other half of the same surface (docs/sync-progress.md): the background hint, which names the
# accounts a pass nobody started is downloading for. `<account-id>:<done>/<total>` stages the
# folder phase; `<account-id>:<done>` (no denominator, the body warm's real shape) stages the
# body phase. Comma-separated for several. The showcase seed's account id IS its address, so this
# names it on screen.
MAILCAL_FAKE_SYNC_HINT=eva.jansen@example.com:3/12 scripts/dev/boot.sh windows
MAILCAL_FAKE_SYNC_HINT=eva.jansen@example.com:2022 scripts/dev/boot.sh macos

# Every platform: come up light or dark whatever the machine is set to (`system` forces the
# machine's own setting back on). It pins how the run STARTS; a pick in Settings still wins for
# the rest of the session, and nothing is persisted. Android has no env vars, so build-and-run.sh
# turns it into the same intent extra the other hooks use.
MAILCAL_APPEARANCE=dark scripts/dev/boot.sh macos
```

`MAILCAL_APPEARANCE` exists because the setting it overrides is an **app-level** one
(`docs/settings.md` → General, persisted in the core), so photographing or asserting on both themes
used to mean changing the OS. Windows' `control.ps1` deliberately does not clear it between verbs:
it decides only which colours the window is painted in, so a whole UI suite can be driven in dark.

**The store-screenshot run sets it itself, so it is not a hook to pass there.**
`scripts/dev/showcase.sh` pins every capture to an appearance: light for each screen, plus a second
capture of the mailbox list in dark (`<locale>-list-dark.png`), and exports the variable per shot,
so an ambient one from your shell is overwritten rather than obeyed. That is deliberate: a set left
to inherit the machine's theme is a set whose appearance depends on who ran it, and nothing
downstream can tell. See [`store-screenshots.md`](store-screenshots.md).

**The `MAILCAL_*` hooks are DEBUG-only** (`#if DEBUG` in `MailboxModel.Debug.cs`; the core's own are
`cfg(debug_assertions)`), so they compile out of a release binary. Where a launch behaviour is a real product affordance rather than a test
convenience, it ships as a **command-line flag** instead. So far there is one: **`--calendar`** (or
`/calendar`) opens the app straight on the calendar grid: `Mailcal.exe --calendar`. It works in
every configuration, honours a second launch of the already-running app (a shortcut, a tile), and is
the release-safe twin of `MAILCAL_CALENDAR`. Both end at `MainWindow.ShowCalendarSurface`, so the
grid opens on today, scrolled to now. This is what a "Calendar" Start-menu shortcut or a jump-list
entry would point at.

### Correction: WinUI touch **can** be synthesized (2026-07-13)

This file, and the `verify-windows-ui` skill, used to say a WinUI gesture "needs real touch/pen/
precision-touchpad input, **which cannot be synthesized**", and `MAILCAL_SWIPE` was built to route
around that. **The wall is not there.**

Win32's `InitializeTouchInjection` / `InjectTouchInput` inject at the *pointer-device* level, so the
OS delivers them as **genuine touch**: they drive the real gesture pipeline (`SwipeControl`,
`ScrollView`, and the calendar's own pointer owner) from an ordinary, **unelevated, unpackaged**
process. No capability, no package identity, no elevation, no Store approval. Verified against a
packaged WinUI 3 app, and used to drive the calendar grid's swipes and pinches end-to-end.

The confusion is understandable: the *WinRT* API everyone finds first
(`Windows.UI.Input.Preview.Injection.InputInjector`) **does** need the `inputInjectionBrokered`
restricted capability *and* package identity, which the unpackaged dev loop does not have. The Win32
one needs neither.

Use [`clients/windows/touch.ps1`](../clients/windows/touch.ps1): `Initialize-Touch`,
`Invoke-TouchFlick`, `Invoke-TouchDrag`, `Invoke-TouchPinch` (including a **diagonal** two-finger
pinch, which a touchpad cannot produce). Its header documents the five traps, each of which costs an
afternoon: `POINTER_FLAG_NEW` on the DOWN frame fails with error 87; coordinates are *physical*
screen pixels so the injector must be DPI-aware; the UP frame must be at the same point as the last
UPDATE; frames must be paced; and it is a **real finger**, so it goes to whatever window is under the
point: assert what is on screen *before* injecting.

**But do not mistake it for the test that matters.** A script cannot land a gesture one frame into
the previous gesture's *animation*, and that race is where the calendar's swallowed-swipe bug lived,
so synthetic touch is testing the case that already worked
([`docs/calendar.md`](calendar.md) §9). The race is covered by `clients/windows/Mailcal.Tests/CalendarFlickTests.cs`,
which owns the clock. Use both; neither replaces the other.

The swipe hook leaves the undo window open for ~4s: screenshot (or press Undo) within it to see the
deferred state, or wait it out to see the action actually dispatched.

For direct input where a stable CLI exists:

```sh
scripts/dev/control.sh android tap <x> <y> | text "<s>" | key back | swipe ... | ui-dump
scripts/dev/control.sh iphone ui-dump | find "<label>" | press "<label>" | probe <x> <y> | tap <x> <y> | text "<s>"
scripts/dev/control.sh macos  tap <x> <y> | text "<s>" | key return | find "<label>" | ui-dump
scripts/dev/control.sh windows open-first | calendar | home | swipe <delete|archive|star> | ui-dump
scripts/dev/control.sh linux activate "<accessible name>" | find "<accessible name>" | set-text "<accessible name>" "<value>" | ui-dump
```

`ui-dump` prints the accessibility tree (UI Automation on Windows) so you can find nodes.

**Linux** is driven through GTK's AT-SPI tree. `activate` exact-matches a localised accessible name,
requires the control to be sensitive and visible in the live tree, and invokes only a semantic
`activate` / `click` / `press` / `open` action. It deliberately has no coordinate fallback. GTK
editable controls use AT-SPI's editable-text interface through `set-text`. GTK list rows do not
expose a dependable action, so establish reading state with the debug-only exact subject hook
instead:

```sh
MAILCAL_OPEN_SUBJECT="HTML message with a remote image" scripts/dev/boot.sh linux
scripts/dev/control.sh linux activate "Load images"
```

**macOS** is driven through the Accessibility API + `CGEvent`
([`scripts/dev/macos-ax.swift`](../scripts/dev/macos-ax.swift)). Drive it **by label, not by pixel**:
`find` resolves a label to coordinates that pipe into `tap`, so a flow survives a layout change:
`control.sh macos tap $(control.sh macos find "Reply")`. Its `ui-dump` is also the **assertion
oracle**: it reports what the app actually shows, so a flow can be checked by grep instead of by eye
(an `AXSheet` node means a dialog is up; a text field's value proves a draft survived). This needs
**Accessibility permission for the terminal's host app** (System Settings → Privacy & Security →
Accessibility): without it, AX reads return empty and clicks are swallowed *silently*, so
`control.sh macos` fails loudly (exit 3) rather than looking like the app ignored the input. Known
limit: SwiftUI **`.swipeActions` don't fire from synthetic events** (that gesture needs a real
trackpad): ask the user to test a swipe by hand rather than concluding it's unsupported.

**iPhone/iPad** is driven through Meta's [idb](https://fbidb.io), and reads the same way macOS does,
by label, never by pixel. `describe-all` frames and `idb ui tap` are both in **points**, so a
label resolves straight into a tap with no scaling and no Simulator-window arithmetic;
[`scripts/dev/ios_ui_idb.py`](../scripts/dev/ios_ui_idb.py) prints the tree in the same
`Role [x,y] Label` lines the macOS dump uses, so a grep ports between them:

```sh
scripts/dev/control.sh iphone ui-dump | grep AXButton
scripts/dev/control.sh iphone press "Accept this invitation"   # find + tap in one call
scripts/dev/control.sh iphone tap $(scripts/dev/control.sh iphone find "Reply")
```

Nothing has to be frontmost on the Mac (unlike macOS's `CGEvent` path), and no Accessibility
permission is involved. Every call resolves the **booted simulator** via `simctl` and starts its
`idb_companion` if absent: an unqualified `idb` call dies with *"No udid provided and there are
multiple companions"* the moment a second simulator has ever been used, which is why this must go
through `control.sh` rather than a hand-rolled `idb` invocation. Because the udid comes from
`simctl`, it can only ever drive a simulator; a physical device is
[`scripts/dev/device.sh`](../scripts/dev/device.sh)'s job.

> **Known gap: the navigation bar does not enumerate.** `describe-all` reports the top bar as one
> unlabelled `AXGroup [220,89]` and omits its items, so `find`/`press` cannot reach Compose, More,
> Send or Cancel. They *are* in the accessibility tree and VoiceOver reaches them (`probe` returns
> a fully labelled `AXButton 'Compose'` for the same pixel), and idb stops the same way in Apple's
> own Settings app. So **a toolbar item missing from a dump is not a finding about the app.** Use
> `control.sh iphone probe <x> <y>` to identify a top-bar control, then `tap` it, or reach the state
> with a `MAILCAL_*` launch hook.

**Assert VoiceOver reachability, not pixels.** This is the check a screenshot structurally cannot
do, and its absence let a real bug ship: the invitation card set `.accessibilityLabel` on a
`.accessibilityElement(children: .contain)` container, which macOS keeps expanded but **iOS
collapses into a single node**: VoiceOver read "Meeting invitation from Bob Tester" and stopped,
with no Accept, Maybe or Decline reachable. The buttons are perfectly visible in a screenshot. The
same two commands say otherwise on either platform:

```sh
scripts/dev/control.sh iphone ui-dump | grep -cE "Organiser|When|Where"   # 0 on a collapsed card
scripts/dev/control.sh macos  ui-dump | grep -cE "Organiser|When|Where"   # macOS keeps the children
```

So when a change adds or wraps a container: dump the tree on **iOS as well as macOS** and confirm
the controls inside are still their own nodes. A difference between the two platforms is the signal.

**To *assert* on the Windows UI** (rather than eyeball a screenshot), dot-source
[`clients/windows/uia.ps1`](../clients/windows/uia.ps1): it carries the UI Automation walk and the
match rules, and the **`verify-windows-ui` skill** explains when to reach for what. Read that header
before writing a verification script: several of WinUI's automation traps produce a *green assertion
for something that is not on screen*, and a hand-rolled tree walk will hit them.
**Windows** is driven the same way: synthetic pixel input doesn't reliably drive WinUI, so
`control.sh windows` uses the `MAILCAL_*` launch hooks: each verb relaunches the built exe into a
known state (the app is single-instanced, so a hook needs a fresh process). `home` also re-syncs
the INBOX on reconnect, so on the JMAP account it's how you pick up mail added with
`harness.sh deliver`; on `stalwart-imap` the IDLE push delivers it to the running app instead.

## 6. Physical iOS device: background sync + notifications

Background delivery **cannot be tested on a simulator**: `BGTaskScheduler` never runs there and
notification banners don't render. Use a **real iPhone/iPad**, driven by `scripts/dev/device.sh`
(the `ios-device-bgsync` skill). Merely *running* the app on the device is the ordinary dev loop:
`clients/apple/Scripts/build-and-run.sh --iphone` targets a connected one on its own; what follows
adds the pull-the-log and trigger-a-pass verbs that loop has no reason to carry:

```sh
scripts/dev/device.sh doctor          # device + Developer Mode + signing team: run this first
scripts/dev/device.sh all             # build + install + launch; then add a real account in the app
scripts/dev/device.sh bgsync          # 1st pass → SEEDED (no notification, by design)
# …new mail arrives…
scripts/dev/device.sh bgsync          # next pass → DETECTED → banner on the device
scripts/dev/device.sh logs --grep RE  # pull the on-device log;  marks  prints the high-water marks
```

`bgsync` triggers one pass via the `MAILCAL_RUN_BGSYNC=1` **DEBUG** launch hook (the only
CLI-drivable trigger: an lldb `_simulateLaunchForTaskWithIdentifier` won't connect over the
CoreDevice tunnel from the CLI, and the Darwin `notifyutil` trigger is simulator-only). It prints the
per-account **mark before/after** as the deterministic SEEDED-vs-DETECTED signal.

Gotchas the tooling handles for you: the signing **team** is derived from the cert OU (not the cert
id); **Developer Mode** must be enabled on the device (manual, `doctor` flags it); the device must be
**unlocked** to launch; and it **terminates before launch** so a fresh `onAppear` fires the hook.

The local Stalwart harness is **loopback-only**. Android physical devices and emulators use
`adb reverse` from `scripts/dev/boot.sh android`, so the debug app dials `127.0.0.1` on-device and
reaches the host harness. iOS physical-device testing still uses a **real stored account** and real
inbound mail. For a fully self-serve loop on a **simulator/emulator** instead, pair
`MAILCAL_RUN_BGSYNC` (iOS) / the `DEBUG_RUN_SYNC` broadcast
(`adb shell am broadcast -a eu.allodia.mailcal.DEBUG_RUN_SYNC -p eu.allodia.mailcal`, Android)
with `harness.sh deliver` to inject the new mail. Full contract: [`background-sync.md`](background-sync.md).

## 7. Testing Apple against your **real** accounts: the Keychain trap

Debugging a *real* account on macOS (a provider bug, an expired grant) means running against
`~/.local/share/mailcal` and the real Keychain rather than the harness. There is one trap on that
path, and it produces a **wrong diagnosis** rather than an error:

> **A dev-signed build silently reads only the Keychain items it already owns.** macOS binds a
> keychain item's ACL to the code signature that created it, so `build-and-run.sh --macos` (signed
> "to run locally", then re-signed with an *Apple Development* identity) cannot read the items your
> installed **Developer ID**-signed app wrote. It does not prompt and it does not fail: the reads
> come back empty and the app boots with **fewer accounts**, exactly as if you had never added them.

Observed 2026-08-01: a dev build came up with **1 of 5** accounts and `refresh_mail: syncing 1
account(s)`, while `security dump-keychain | grep -c '"svce"<blob>="eu.allodia.mailcal"'` reported
**8** stored configs. Worse than the missing four: the one it *did* read was a **stale copy the dev
build had written itself** on some earlier run, so a perfectly healthy Google account presented as
`invalid_grant — Token has been expired or revoked` and was briefly diagnosed as a provider-side
token expiry. The account was fine; the credential was old.

**Use a build whose signature matches the installed app:**

```sh
clients/apple/Scripts/package.sh --no-notarize      # Developer-ID signed, no notary round-trip
open clients/apple/build/package/export/AllodiaMail.app
```

Notarization is irrelevant here: the ACL keys on the **signing identity**, so `--no-notarize` is
enough and skips a slow remote step. Gatekeeper accepts it on the build machine. Run it from the
export directory rather than installing over `/Applications`, so the app you actually use stays put
and you can fall back instantly. Confirm before you trust a run:

```sh
codesign -dv --verbose=2 <app> 2>&1 | grep Authority=Developer   # must match the installed app's
grep -E "NewAccounts|syncing [0-9]+ account" ~/.local/share/mailcal/mailcal.log | tail -2
```

**Two accounts of the same address can coexist in the Keychain** (one per signing identity), which
is why 5 accounts had 8 items. Prune the stale ones by hand when they accumulate; nothing in the app
distinguishes them.

Finally, quit the running app before starting another build against the same store: both open the
same SQLite file, and `~/.local/share/mailcal` is the **real** one.

## Build time and disk

The repo's own defaults are already tuned, and the first subsection says how and why. What follows
it is **per-machine**: adopt what your box needs.

### `cargo clean` is a symptom, not a maintenance task

**If you are cleaning to free disk, something is configured wrong: find it instead.** A clean throws
away the cache between a 10-second and a 3-minute rebuild. What was wrong here, and stays fixed:

⚠️ **That holds for a *healthy* target dir, and stops holding once one has bloated.** Measured in
the engine repo, same commit, same trigger of 19 recompiled crates: a **34 GB** target on a disk at
**96%** took **479 s** (83 s of it userland, 239 s in the kernel, so it was waiting rather than
building), and the same thing after `rm -rf target` took **21.7 s**. A *cold* build of all 189
crates from empty took 57 s, which makes the bloated warm cache **8× slower than no cache at all**.
Past that point a full reset is the fastest available move, not a symptom. Check `du -sh target`
before believing the paragraph above.

- **Debug info in `[profile.dev]` *and* `[profile.test]`**: our crates at `line-tables-only`,
  dependencies at `debug = 0`. It was 2.8 GB of PDBs and more than half the inner loop; backtraces
  still name a file and line. It lived in `ci.yml` as `CARGO_PROFILE_DEV_DEBUG` for a year, which
  fixed it only for the runners: **a build fix in the workflow file is a fix nobody who builds
  gets.**

  **`[profile.test]` has to repeat it, and for a year it did not.** `cargo test` builds its targets
  under the *test* profile, and although the Cargo book describes that profile as inheriting `dev`,
  the top-level `debug` key measurably does not come with it: setting it on `dev` alone rebuilds
  nothing for `cargo test`, and adding `[profile.test]` rebuilds everything. So the fix applied to
  `cargo build` and never to `gate.sh`, the command that does essentially all of the building
  anyone waits on. Measured in the engine repo, `cargo test --workspace --all-features --no-run`
  after touching one crate: **368s before, 149s after, twice.** The `package."*"` override *is*
  inherited and needs no twin, which is exactly why dependencies were covered and our own crates
  were not.
- **`cargo rustc --crate-type`, not a third `[lib] crate-type`.** Crate-type is not per-target, so a
  declared iOS `staticlib` made Windows, Android and Linux each build a 1.9 GB archive only
  [`build-core.sh`](clients/apple/Scripts/build-core.sh) opens. A disk fix, not a time one.
- **Duplicate toolchains**: check `rustup toolchain list -v` and compare `rustc +<name> --version`;
  several ~1.5 GB copies can resolve to the same rustc. Careful uninstalling `stable`: the sibling
  engine repo has no toolchain pin and builds on it.
- **The incremental cache, which no cargo reclaims.** Cargo keeps a session directory per
  compilation context and prunes none of them (every engine re-pin and every local-engine
  `[patch]` toggle mints a fresh set), and its `clean gc` collects `$CARGO_HOME`, not `target/`.
  Two days of one branch left 20 GiB. A **green** [`gate.sh`](scripts/dev/gate.sh) now drops it past
  a 5 GiB cap. "Nine seconds on the next rebuild" is the cost of dropping a *small* one: dropping
  a 3.6 GiB cache by hand cost **70 crates recompiled and 323 s**, so it is a disk trade, not a free
  one. Do **not** reach for `cargo clean` instead: that takes `deps/` with it, which is the rebuild
  this section is about.
- **Stale test binaries, which no cargo reclaims either.** Every rebuild links a new test executable
  under a new hash and leaves the old one; a day of iterating left **434 of them, 5.7 GB**. They
  cost disk, and on macOS they cost more than that (see below). Deleting executables in
  `target/debug/deps` older than a couple of days is safe (worst case is a relink); deleting the
  `.rlib`s beside them is not, because that rebuilds every dependency.

**On macOS, check Gatekeeper before you profile anything.** `syspolicyd` assesses every locally
built executable the first time it runs (hashing the whole file), and `XProtectService` scans it
too. A `cargo test --workspace` that links ~96 test binaries therefore pays ~96 first-run
assessments, and none of it appears in your own process: low CPU, constant disk reads, and a
profiler that shows nothing. One measured full suite: **35.8 minutes wall for 19.3 seconds of
actual test execution.**

The fix is Apple's own developer exemption, and it is per-machine, so it is written here rather than
fixed in a file: **System Settings → Privacy & Security → Developer Tools**, add the app that
*hosts* the build (the terminal, or the editor if the build is launched from one), then restart it.
The exemption follows the responsible process, so granting it to the host covers cargo, rustc and
every test binary underneath; there is nothing to register per binary, which is just as well since
their hashes change on every build. It stops assessment of what that app spawns; it does not
disable Gatekeeper, and `spctl --master-disable` is not the answer.

Same repo, same commit, same 19 crates rebuilt, before and after granting it: **35.8 min → 7.8 min.**

Per-machine wins that belong in no tracked file (`lld-link.exe`, a shared `CARGO_TARGET_DIR` for
worktrees): this document → "Build time and disk".

### The incremental cache is 20 GB, and nothing reclaims it

`target/*/incremental` is pure cache: delete it whenever you like. What is surprising is how fast
it grows and that nothing prunes it: two days of work on one branch left **20 GiB** across 699
session directories, none of them older than those two days.

Cargo mints a session directory per distinct *compilation context* and keeps every one. A context
changes with the dependency graph, so each engine re-pin and each toggle of the local-engine
`[patch]` override above mints a fresh set (~430 MB for `mailcal-app` alone, every time). This is
churn, not accumulation, which is why deleting "old" directories reclaims nothing.

**Cargo cannot do this for you.** `cargo clean` has no stale, age or size option, and the
`cargo clean gc` on nightly `-Zgc` is, per its own help text, "Clean global caches":
`--max-src-age`, `--max-crate-age`, `--max-index-age`, all of them `$CARGO_HOME`. So is stable's
`cache.auto-clean-frequency`. Here that whole global cache is 1.6 GB and its GC reports zero files;
the 20 GiB is in `target/`, which no version of cargo garbage-collects.

So [`gate.sh`](../scripts/dev/gate.sh) does it: a **green** gate drops the cache once it is past a
5 GiB cap. A red gate leaves it alone, because that is when you are still iterating and the cache
is worth its disk. What that costs, measured on an M-series Mac:
`cargo build -p mailcal-bindings` after a one-line edit in `mailcal-app`:

| | rebuild |
|---|---|
| cache warm | 3.3s |
| first rebuild after a prune | 12.0s |
| every rebuild after that | 3.4s |

Nine seconds, once, per chunk of work. To reclaim it by hand at any time:

```sh
rm -rf target/*/incremental        # safe, always; costs one non-incremental rebuild
du -sh target/*                    # where the rest of it went
```

Note what is **not** in that command: `target/debug/deps`. That is the compiled dependency graph,
and dropping it is the three-minute rebuild the first subsection above is about.

### Link with lld

Linking dominates the inner loop on Windows. `link.exe` is the default; LLVM's `lld-link.exe` is
markedly faster and ships with Visual Studio's optional *C++ Clang tools for Windows* component.
Measured on a Surface Pro X (SQ1, arm64): one line changed in `mailcal-app`, then
`cargo build -p mailcal-bindings`:

| linker | whole loop | `mailcal-bindings` unit |
|---|---|---|
| `link.exe` | 10.60s | 7.26s |
| `lld-link.exe` | **7.96s** | **5.20s** |

It cannot be tracked: a box or runner without that VS component fails to link at all. So put it in a
`.cargo/config.toml` in the directory **above** your checkouts: Cargo merges config from every
ancestor directory, the same mechanism the engine path override uses
([`.cargo/config.toml`](../.cargo/config.toml)):

```toml
# D:\repos\.cargo\config.toml: applies to every checkout beneath it, committed to none
[target.aarch64-pc-windows-msvc]
linker = "lld-link.exe"
[target.x86_64-pc-windows-msvc]
linker = "lld-link.exe"
```

Set `linker` and **nothing else** in that file. A `rustflags` key for the same triple would *shadow*
the repo's rather than merge with it, silently dropping `-C target-feature=+crt-static`, the flag
whose absence failed Store certification. After adopting it, and after any Visual Studio update,
re-run the gate that would catch exactly that:

```powershell
. .\clients\windows\rust-crt.ps1
Assert-StaticCrt -Dll .\target\debug\mailcal_bindings.dll
```

(Verified passing under lld on 2026-07-31.) If `lld-link` ever goes missing, delete the file: the
build reverts to `link.exe` and is slower, never wrong.

### Worktrees each grow their own `target/`

A `.claude/worktrees/*` checkout is a separate Cargo workspace, so it builds a full target dir of its
own, several GB per worktree, none of it shared with main. Point them at one directory
(`CARGO_TARGET_DIR=D:\repos\.cargo-target`) and they share a cache instead. The trade is real and
worth knowing: Cargo takes a **lock** on the target dir, so two builds in two worktrees serialize
rather than run at once.

## Known gaps / follow-ups

- **The Linux acceptance run drives no analytics consent and no Diagnostics.** Mail actions,
  search, the calendar, invitations, contacts and signatures are all driven; those two are not.
  Verify them by hand for now.
- **There is no `--set docs` driver for Linux.** `doc_screens_for` in
  [`showcase.sh`](../scripts/dev/showcase.sh) offers it none: the four `setup-*` moments are driven
  by an address the client types, and the Linux setup flow takes no seeded one.

- **Android tablets are single-pane.** The client has no `WindowSizeClass`/list-detail layout, so a
  tablet renders the phone UI at full width, with no reading pane. `showcase.sh android-tablet-7` /
  `android-tablet-10` shoot it fine and fill both Play slots; the layout itself is the follow-up.
  It shows most on the **10-inch** `list` and `settings` captures, where the content runs out
  around two-thirds down and the rest is empty: the calendar and reply screens fill the height.
  Closing it needs a list-detail layout, not a screenshot change.
- **A tablet AVD the run booted may lose its app install.** The emulator is started with
  `-no-snapshot` and stopped with `emu kill`, and an install has been observed not to survive that
  round trip, so a later `--no-build` run against the same AVD can find no activity to start. It
  fails loudly (the showcase interlock sees an empty log slice); re-run without `--no-build`.
- **macOS showcase PNGs keep an alpha channel** (rounded window corners, from `screencapture -o
  -l`). Flatten onto an opaque background before uploading if the store rejects transparency.
- **Windows control is launch-hook only.** `control.sh windows` drives the app by relaunching it
  into a known state via the `MAILCAL_*` hooks (reliable, layout-independent); there is no pixel
  `tap`/`text`/`swipe`, because synthetic input doesn't drive WinUI dependably. `ui-dump` (UI
  Automation) is read-only, for discovery. Note `home`'s re-sync is a **JMAP** workaround: over
  `stalwart-imap` new mail arrives by IDLE without relaunching anything.
- **Send/SMTP** is not exercised against the harness (its SMTP is plaintext; the core submits over
  implicit TLS). Test compose/send against a personal account for now.
- **Apple and Android dev runs share the real preferences.** Their persisted choices live in
  app-wide `UserDefaults` / `SharedPreferences` (e.g. `diagnostic_log_debug_enabled`), which a
  `MAILCAL_DEV_ACCOUNT` run does not isolate, so flipping the Diagnostics DEBUG toggle in a
  harness test there persists for the developer's real app until flipped back. Windows isolates its
  preference files into the dev store subdir (see §2); doing the same on the other platforms is the
  follow-up.
- **Calendar over `stalwart-imap`** is not included (IMAP carries no calendar); use `--account
  stalwart` (JMAP) for calendar.
