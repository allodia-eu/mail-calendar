---
name: verify-windows-ui
description: Drive and ASSERT on the running WinUI (Windows) client from a script: find and read through UI Automation (clients/windows/uia.ps1), press through Microsoft's winapp CLI (winapp.ps1), read the composer's message body over DevTools (composer.ps1), and paste real Word and Excel clipboards (office-clipboard.ps1). Use when verifying that a Windows UI change actually works, not just that it compiles. Covers the traps that produce silent FALSE PASSES, and which dataset proves what.
---

# verify-windows-ui: prove a Windows UI change actually works

Windows-only (needs the built client and a desktop session). To *boot* a client, see **debug-app**;
this skill is about **asserting** on it afterwards.

Two instruments, one job each, and **neither is reimplemented in a script**:

- **Finding and reading** is UI Automation, in-process:
  [`clients/windows/uia.ps1`](../../../clients/windows/uia.ps1). Its header documents traps that a
  hand-rolled walk will hit, and three of them hand you a **green assertion for something that is
  not on screen**. That is worse than a crash, because you believe it and move on.
- **Pressing** is Microsoft's `winapp` CLI (`winget install Microsoft.WinAppCLI`, 0.7 or later),
  through [`clients/windows/winapp.ps1`](../../../clients/windows/winapp.ps1), which `uia.ps1`
  dot-sources. Every click, double-click, right-click, hover, drag, wheel and keystroke goes through
  it: it is DPI-aware and injects real pointer and keyboard input, so a press lands where it was
  aimed at any scale, a flyout opens, a tooltip rises. A suite does not P/Invoke `mouse_event` or
  call `SendKeys` of its own; the three places that still do say why, in place.

Reading does not move to `winapp` too, and `uia.ps1`'s header says why with the numbers: each
`winapp` verb is a process of its own, a text selector is a substring match, and while a composer
is open `winapp`'s walk of the window never ends.

## 0. Prerequisites

```powershell
./clients/windows/build-and-run.ps1 -NoRun     # build first; assertions run against the built exe
winget install Microsoft.WinAppCLI             # 0.7 or later, once: every press goes through it
scripts/dev/harness.sh up                      # only if you need a REAL transport (see §3)
```

## 0.5. Before writing a one-off script, check whether it belongs in the suite

There is a standing UI test suite, [`clients/windows/uitests/`](../../../clients/windows/uitests),
run with `./clients/windows/uitests/run-ui-tests.ps1`. It is the same UIA primitives as below, with
a runner that launches the dataset, reports pass/fail, and exits non-zero.

Suites declaring the same dataset and the same `Env` values **share one launch**, so yours may not
assume a fresh app, and may not read a top-level variable it did not define itself. Both rules, and
what the runner does between suites to make them safe, are in that script's header: read it before
adding a file.

**If what you are about to verify is a rule rather than a one-off question, add a case there
instead.** A rule proven once by hand is a rule that regresses silently the next time; the whole
reason the suite exists is that `Mailcal.Tests` cannot link anything WinUI-flavoured, so bindings,
projected properties and initial control state have no other machine watching them.

A suite file declares its dataset and its cases and nothing else:

```powershell
$Suite = @{
  Dataset = 'showcase'          # or 'first-run'; a harness suite goes in Mailcal.UiTests (§3)
  Env = @{ MAILCAL_… = '…' }    # optional; set before launch, removed after the suite
  Cases = @(
    @{ Name = 'what must be true'; Body = { Assert-Equal 600 (Get-RowTextWeight -Row $r -Text 'x') 'why' } }
  )
}
```

**`Env` is how a suite reaches a `MAILCAL_*` switch**, and both halves of the runner's handling
matter: the app reads them once at startup, so a `$env:` line inside a Body is already too late,
and one set at a suite file's top level leaks into every suite that sorts after it (§5's
`MAILCAL_SHOWCASE` trap, which reads as a passing test against the wrong dataset). A harness suite
in [`Mailcal.UiTests`](../../../clients/windows/Mailcal.UiTests) does the same through its launch
fixture, which is also how it picks a transport: `MAILCAL_DEV_ACCOUNT=stalwart-imap` for the CalDAV
half.

**When the state you need is one the harness cannot produce, fake the *input*, never the surface.**
[`InvitationReplyPromptTests.cs`](../../../clients/windows/Mailcal.UiTests/Suites/InvitationReplyPromptTests.cs) is the worked example: no server here ever reports a failed iTIP
delivery, so `MAILCAL_FAKE_REPLY_DELIVERY=failed:5.2` substitutes the calendar server's verdict in
the **core** and leaves everything downstream real. A hook that instead set the view-model's prompt
directly would go on passing after the wiring between core and client was cut, which is the one
class of bug this whole suite exists to catch.

**A new case is not finished until you have watched it fail.** Break the thing it covers, run the
suite, confirm *that* case goes red for *that* reason, then put it back. A UI assertion is unusually
easy to write in a form that cannot fail, scoped to the wrong element, or comparing two constants
that happen to match, and it will report a confident green forever.

## 1. The loop

```powershell
cd clients/windows
./control.ps1 home                 # relaunch into a known state (single-instanced: hooks need a fresh process)
. ./uia.ps1                        # dot-source the primitives

$rows = Get-MailRows                                  # scoped to the message list, NOT the sidebar
Invoke-UiaElement $rows[0]                            # opens it (Invoke, not Select: see below)
$send = Wait-UiaElement -AutomationId 'SendButton'    # poll; never a fixed Start-Sleep
Set-UiaText (Find-UiaElement -AutomationId 'ToBox') 'someone@example.com'
Invoke-UiaElement (Find-UiaElement -Name 'Discard' -Type Button)
Wait-UiaGone -AutomationId 'SendButton'

Invoke-WinappClick $rows[1] -Right                    # found here, pressed by winapp: the row's flyout
Send-WinappKeys esc                                   # real keystrokes, so accelerators fire
Invoke-WinappHover (Find-UiaElement -AutomationId 'SelectionClear' -Type Button)   # raises its tooltip
```

**Pattern before pointer.** Where an automation pattern does what the user does (Invoke, Toggle,
Select, Set), use it: it is milliseconds and needs no window in front. Reach for a `winapp` press
when the thing under test *is* the pointer or the keyboard: a flyout, a double-click, a drag, a
hover, an accelerator, a drawn surface with no patterns.

Discovery: `./control.ps1 ui-dump` prints the live tree (control type / name / `#automationId`).
Start there: assert on `AutomationId` where one exists, since names are localised.

## 2. The traps (why you dot-source rather than hand-roll)

| Trap | What it looks like | The fix |
|---|---|---|
| `FindFirst`/`FindAll` with `TreeScope.Descendants` **under-walks** WinUI's tree | Returned 90 elements and missed an *entire open composer*. Looks like the feature is broken. | `Get-UiaTree` does a recursive **children** walk |
| Matching on **`-Name` alone** also matches the inert `TextBlock` inside a control | `-Name Archive` returned the reading pane's *label* and PASSed for a menu item that did not exist | **Always pass `-Type`** for anything you intend to press |
| A bare `ListItem` sweep also matches the **sidebar** entries | "Click row 3" opened *Add account* and hid the whole shell: reads as "the surface under test closed" | `Get-MailRows` |
| Rows open on `ItemClick`, which `SelectionItem.Select()` never raises | Row highlights; message never opens | `Invoke-UiaElement` (prefers `Invoke`) |
| A **category list** (the Settings dialog's General/Calendar/… source-list) switches on **selection**, and `Invoke-UiaElement` calls `Invoke` first: a no-op for a `ListViewItem` | Clicked "Calendar"; the detail pane stayed on "General". Looks like the category is broken | Call `SelectionItemPattern.Select()` directly on the item |
| `Find-UiaElements -Name X` returns **duplicates** across shells: a Settings dialog category and the **sidebar** nav item can share a name (`Calendar`); order is not "dialog last" | `$items[-1]` grabbed the sidebar entry and navigated the app *behind* the modal: no visible change, reads as "nothing happened" | Disambiguate by `Current.BoundingRectangle.X` (dialog content is inset), not by array index |
| A `ToggleSwitch` supports **neither** `Invoke` **nor** `SelectionItem`, and `Toggle()` *cycles* rather than sets | `Invoke-UiaElement` throws on one. Worse, a blind `Toggle()` on an already-on switch turns it **off**: a consent test "opts in" and actually **withdraws** | `Set-UiaToggle $el -On` (reads `ToggleState` first; idempotent), `Get-UiaToggle` to assert |
| `BoundingRectangle` is in **physical** pixels; every size in XAML is in **device-independent** ones | At 200% a `MaxHeight="176"` panel measures 352 and a cap assertion fails on a change that is correct. The lenient direction is worse: a floor of 120 silently accepts 60 | `ConvertTo-UiaPixels <dip>` before comparing against any constant taken from XAML |
| A bare layout **Grid**/**StackPanel** gets no automation peer, so an `AutomationProperties.AutomationId` on one reaches nothing | The wait times out and reads as "the pane never rendered", and no timeout is long enough, on any display | Measure through a **control** inside it (`ScrollViewer`, `TextBlock`), whose `x:Name` is already its AutomationId |
| Reading `BoundingRectangle` straight off an element that is collapsed or scrolled off the surface | Infinities on both sides of a comparison, and a confident **PASS** about something nobody can see | `Get-RenderedBounds $el -What '<name>'` |

Four of these fail **green**. If an assertion passes on the first try and you are surprised, check it
is not one of these.

## 3. Which dataset: this decides whether your test proves anything

| | accounts | engine | use it to |
|---|---|---|---|
| **showcase** (`showcase.ps1 -Locale en -Screen <list\|reply\|settings\|add-account>`) | **two** | in-memory, deterministic | exercise multi-account UI, and any screen that needs no real mail action |
| **harness** (`MAILCAL_DEV_ACCOUNT=stalwart`, the default in `control.ps1`) | one | **real transport** | prove a destructive action or a send **actually landed** |

**The showcase engine does not really perform mail actions.** A committed archive/delete there proves
only that you dispatched into a void: the row even reappears when the commit-grace un-hide fires,
because the core never removed it. Verify anything destructive against the **harness**.

## 4. What UIA cannot reach, and what to use instead

### The pointer and the keyboard: `winapp`, through `winapp.ps1`

`Invoke-WinappClick` (with `-Right` or `-Double`), `Invoke-WinappHover`, `Invoke-WinappDrag`,
`Invoke-WinappWheel` and `Send-WinappKeys` each take the element a suite already found and press it.
Each costs about 2 s, most of it `winapp` looking the element up again, so put presses where the test
needs them, never in a polling loop. Three traps, each of which pressed the wrong thing or nothing at
all; `winapp.ps1`'s header has the detail:

- **A text selector is a substring match**, and `click`, `hover` and `invoke` take no `--root` or
  `--type`. `winapp ui click 'Allodia Mail & Calendar'` pressed the mail row "Welcome to Allodia
  Mail & Calendar". The bridge never presses by text: it matches name, control type **and** bounds,
  and fails unless exactly one element answers.
- **`-a <pid>` picks the foreground window** when the app has more than one. The bridge names the
  element's own window with `-w`; a flyout's items resolve through it too.
- **While a composer is open, `winapp`'s walk of that window never returns.** It finds a cycle
  through the editor's WebView2 (the WebView2 lists the window's own `InputSiteWindowClass` among its
  children) that UI Automation does not have. A lookup that finds its element returns in seconds; a
  search for every match, or for something absent, does not return at all. The bridge caps every
  call and says so. Keystrokes need no lookup, so `Send-WinappKeys` types into a composer; a click
  into the body uses `Invoke-ClicksAt` (`uitests/appwindows.ps1`).
- **…and until the editor's browser window has gone, which can be after the composer has left the
  screen.** So a suite closes a composer with `Close-Composer` (`uia.ps1`): Discard by id, the
  question answered, and a wait on the editor's own window handle. It throws if anything is left. A
  close that matches a label finds nothing when the label is not there, leaves the composer to the
  runner, and the next suite's first lookup times out in a suite that never opened one. The runner
  counts the window's WebView2 hosts between suites for the same reason (`dataset.ps1`), and says
  what it found when it relaunches.

What still uses in-process input, and why: `Search.Tests` times the moment of one keystroke, which a
`winapp` call cannot place inside its own process; `CalendarWheel.Tests` needs a horizontal wheel,
which `winapp` does not have; touch is below.

For exploring by hand, `winapp ui` on its own is a good instrument: `inspect -i` for the interactive
elements with their slugs, `search <text> --root <selector> --type <type>`, `get-property -p
FontWeight`, `invoke --action toggle-on` (idempotent, and fails rather than falling back),
`screenshot --capture-screen` for a popup (`screenshot.ps1` captures the window, so a menu is not in
it), and `record` for a video. An element with an AutomationId has that id as its selector, stable
across runs; the others get per-run hashes. Read `inspect --json` under `windows[].elements[]`:
`.elements` at the root is `$null`, and an audit over `$null` passes having examined nothing.

### Gestures: inject real touch (`touch.ps1`)

Win32's `InitializeTouchInjection` / `InjectTouchInput` inject at the **pointer-device** level, so
the OS delivers them as genuine touch and they drive the real gesture pipeline (`SwipeControl`,
`ScrollView`, and the calendar grid's own pointer owner) from an ordinary **unelevated, unpackaged**
process. No capability, no package identity, no elevation, no Store approval. (The *WinRT*
`Windows.UI.Input.Preview.Injection.InputInjector` **does** need the `inputInjectionBrokered`
restricted capability *and* package identity, which the unpackaged dev loop has not got.)

```powershell
. ./clients/windows/touch.ps1
Initialize-Touch
$w = Get-MailcalBounds                                   # foregrounds the app; physical pixels
Invoke-TouchFlick -FromX ($w.Right-300) -ToX ($w.Left+300) -Y $w.MidY    # turn the calendar week
Invoke-TouchPinch -CenterX $w.MidX -CenterY $w.MidY -FromSpread 150 -ToSpread 700 -AngleDeg 45
```

`winapp ui touch` does taps, swipes and a pinch along one axis. `touch.ps1` stays for the
**diagonal** pinch, the only way to exercise the grid's two-axis zoom, and for the frame-paced
gestures `calendar-perf.ps1` measures.

Its header documents the five traps (each an afternoon): `POINTER_FLAG_NEW` on the DOWN frame → error
87; coordinates are **physical** screen pixels, so the injector must be DPI-aware; the UP frame must
be at the same point as the last UPDATE; frames must be paced; and **it is a real finger**: it goes
to whatever window is under the point, so assert what is on screen *before* injecting, never after.

**A synthetic gesture cannot test the bug that matters.** It cannot land a flick one frame into the
previous gesture's *animation*, and that race is where the calendar's swallowed swipe lived, so it
tests the case that already worked (`docs/calendar.md` §9). Put that race in a **unit** test that owns
the clock (`Mailcal.Tests/CalendarFlickTests.cs`). Use both.

### The system save/open picker

`FileSavePicker` (the reading pane's *Export as .eml*, Settings → Diagnostics → *Export log…*)
opens the classic Save As, a `#32770` dialog **owned** by the app window. UI Automation files an
owned window under its owner, so a search of the desktop's children never meets it; find it through
the window list (`EnumWindows`, the app's process, class `#32770`). The name box is `#1001` under
`#FileNameControlHost` (the address band carries a `#1001` of its own, which an unscoped search
meets first), and takes a path through its value; Save is `#1`. Set the path rather than typing it:
keys sent as the dialog opens lose their first characters.
[`Mailcal.UiTests/Driver/SaveDialogs.cs`](../../../clients/windows/Mailcal.UiTests/Driver/SaveDialogs.cs)
does all of it. Assert on the **written file** afterwards (bytes, not existence), never on the
dialog: the dialog creates the file empty, and the app writes it afterwards.

### The composer's message body: DevTools (`composer.ps1`)

The body is a page in a WebView2, which UI Automation sees as one group named "Write your message":
whether a paste kept its bold, or where a picture landed, is not in that tree. Launch with the
DevTools port and read the page itself:

```powershell
./clients/windows/control.ps1 home -DevToolsPort 9222   # or showcase.ps1 … -DevToolsPort 9222
. ./clients/windows/uia.ps1; . ./clients/windows/composer.ps1
Reset-ComposerBody '<p>Before |after</p>'               # the caret goes where the | is
Send-WinappKeys 'ctrl+v'
Get-ComposerHtml                                         # the markup the editor now holds
```

`-DevToolsPort` sets `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` for that one launch and removes it,
because the variable is process-wide and every later launch from the shell would open the port too.

**Test a paste with what the source application really writes**, not a clipboard you built:
`clients/windows/office-clipboard.ps1 -From Word` or `-From Excel` puts a formatted document or a
range of cells on the clipboard through the installed Office, and `-Close` afterwards. Word and Excel
write several flavours at once, and their HTML carries markup no fixture writer would think of
(Excel names a cell's bold as a class in a style sheet). Excel renders its clipboard only when asked,
so close it **after** the paste; the script's header has the other traps.

### Microsoft's other WinUI tooling

**Don't adopt the rest of the `win-dev-skills` plugin's workflow.** Its `winui-dev-workflow` assumes
`dotnet new winui-mvvm` and forbids `<WindowsPackageType>None`, which is the opposite of this client
(unpackaged + self-contained for the dev loop, Rust cdylib built first); its `winui-code-review` MVVM
section assumes `CommunityToolkit.Mvvm` ViewModels, which this client does not use (it renders
immutable snapshots from the core), and its theming rule flags `ReadingView.xaml`'s deliberate
literal white message canvas. Its `winui-packaging` is dev-cert grade, not `.msixupload`.

**Its WinUI analyzer is part of the build**, warnings as errors; AGENTS.md ("Toolchain, lints and
CI") says how its rules apply here, the explicit `x:Bind` mode among them.

The `MAILCAL_*` launch hooks (`MailboxModel.Debug.cs`, and the swipe hook in `MailListView.Swipe.cs`)
remain useful as **deterministic** shortcuts past a gesture you are not trying to test, and are
debug-build only.

### Asserting on a DRAWN surface

The calendar grid is a single Win2D canvas, so it has **no XAML tree**: `ui-dump` shows one element
where a week of events is on screen. It exposes its content through a custom `AutomationPeer`
instead, which is what a screen reader reads *and* the only thing a test can see. Walk it:

```powershell
$cond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'CalendarSurface')
$surface = (Get-MailcalWindow).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
# children: HeaderItem per day column, Text per event/band, Button per "+N" chip
```

Each node's `BoundingRectangle` is computed by **the renderer's own geometry**, so it is also how you
check that what is announced is where it is drawn, and it is where a press lands: the nodes have no
Invoke pattern, and `Invoke-WinappClick` on one opens that event, because `winapp` aims at the
published rectangle. (UIA's own `GetClickablePoint()` hit-tests onto the canvas and reports none.)

## 5. Gotchas that are not UIA's fault

- **The app is single-instanced.** An env var only takes effect in a **fresh** process: `control.ps1`
  kills and relaunches for you. Setting `MAILCAL_*` and launching over a running instance does nothing.
- **`MAILCAL_SHOWCASE` leaks process-wide, and a leaked one is a silent dataset swap.** `showcase.ps1`
  sets it with `$env:`, which is per-**process**, not per-scope, so any shell that has taken a
  screenshot leaves it set. A later `control.ps1` relaunch then comes up on the in-memory seed while
  printing `account=stalwart`: the verb says harness, the window shows fiction, and nothing
  disagrees. Since the showcase engine doesn't really perform mail actions, a destructive-action test
  there dispatches into a void and **passes**. `control.ps1` now clears it (2026-07-31, found when a
  showcase suite first sorted ahead of a harness one); if you launch the exe yourself, clear it too.
- **A showcase *capture* is impossible on a 2880×1800 display at 200%.** The pinned 1440×900 logical
  frame is exactly the full screen, so inflating it by the resize border makes the window wider than
  the display, centring lands it at x=-13, and `screenshot.ps1` rightly refuses a shot whose left
  column is off-screen black. That used to block the **whole** UI suite on such a host; the suite now
  launches with `showcase.ps1 -NoCapture`, which keeps both showcase safety asserts and skips only
  the shutter. Store screenshots still need a smaller frame or a different display.
- **Two build output trees.** `build-and-run.ps1` writes `bin\arm64\Debug\…`, while a bare
  `dotnet build -r win-arm64` writes `bin\Debug\…`. Scripts pick the **newest** `Mailcal.exe`, so a
  stray plain `dotnet build` silently becomes the thing under test. Build with `build-and-run.ps1`.
- **PowerShell variables are case-insensitive**: a local `$kids` clobbers a script-scope `$KIDS`.
- **Transient state needs polling, not sleeping.** The undo bar closes after ~4s and the harness
  syncs over the wire; `Wait-UiaElement` / `Wait-MailRowCount` exist so you don't race them.
- **Adding an account through the setup form persists, but under `MAILCAL_DEV_ACCOUNT` it lands in
  an isolated dev namespace, not your real accounts.** A harness run switches the credential store
  to `eu.allodia.mailcal:dev:` (keyed to the dev store subdir: `dev` for `stalwart`, `dev-imap` for
  `stalwart-imap`) via `CredentialStore.UseDevNamespace`, so a form-connect test (JMAP/IMAP) never
  touches, or reorders the index of, the developer's real accounts, and a *normal* launch never
  sees the harness account you added. Form-added dev accounts **persist across dev relaunches** (the
  canned account is injected fresh on top), which is what makes iterating on the setup flow painless.
  To reset, delete the dev namespace **wholesale**: it's throwaway, so unlike the real store you can
  drop the whole index: `eu.allodia.mailcal:dev:account-index` and each
  `eu.allodia.mailcal:dev:account:<id>` (chunks `:1`, `:2`, … if any).
  `clients/windows/clear-dev-namespace.ps1 -Namespace <dev…>` does exactly that, store directory
  included, and never reaches past the namespace's own prefix. Use a *second* seed account
  (`bob@test.local` / `harness-bob-pw`) so the run's own canned account stays untouched. (If you ever
  add through the form in a **non-dev** run, that DOES hit the real `eu.allodia.mailcal:` store and
  its index co-mingles real accounts; there, remove surgically, never nuke the index.)
- **Preference files are dev-isolated too.** Under `MAILCAL_DEV_ACCOUNT` the one-line preference
  files (language, window placement, pane width, the Diagnostics log level) read/write inside the
  dev store subdir (`AppPaths.PrefsDir`), so resizing the window or flipping the Settings →
  Diagnostics DEBUG toggle in a test never changes the developer's real app's next launch. The
  rotating `app.log` deliberately stays **shared**: one file diagnoses whatever ran last, so a
  test that asserts on log content needs no dev-specific path, but must tolerate real-run lines.

## 5.5. When the screen cannot see the bug

Three ways a Windows UI check reports success over a live defect. Each cost a day.

- **A foreground or z-order assertion cannot see a steal that lasts 20 ms.** A window fell behind
  the mailbox because the app activated the mailbox tens of milliseconds after opening it, and the
  app usually had the front back before any screen read landed. A dozen cases asserting "the window
  is in front" passed throughout, on the machine where the fault reproduced every time. **Assert the
  cause, not the symptom**: make the app say what it did at DEBUG level, give the suite
  `Env = @{ ALLODIA_LOG_LEVEL = 'debug' }`, and read `app.log` through `applog.ps1`. A log line is
  timestamped and cannot be missed by sampling. The rule that holds is not "the window is in front"
  but "the mailbox takes no activation of its own after a window opens", stated of the activation
  because that is the thing with a cause; the foreground is only where it shows.
- **A fixture chosen to make one assertion easy can be the one case that does not reproduce.** The
  reading-window suite double-clicked the seeded invitation throughout, because its card guarantees
  the window has content to draw. An invitation's card is native XAML where an ordinary body is a
  document in a WebView2, and every case in the file was exercising the one message type that was
  safe. The case that reads the log now uses an ordinary message for exactly that reason. When
  picking a fixture, ask which one is closest to the risky path, not which one is easiest to assert
  on.
- **The driver is part of the measurement, and it raises the main window.** `Invoke-RowClicks`
  calls `Show-Mailbox` first, and `Close-ExtraWindows` activates the mailbox by closing what was in
  front of it, so a loop that opens a window every 1.5s stamps an activation into the log on a
  cadence that reads exactly like a fault with a fixed delay. Twelve opens in a row "reproduced" a
  steal at 1.47s that way, and the app had logged nothing at all between 0.03s and 1.46s, which is
  what gave it away. Keep the driver out of the observation window: settle well past what you
  intend to read, and read only up to the moment the driver moves again.
- **A mailbox with nothing to do hides faults that a real account shows.** The seeded harness holds
  a static list and reconciles a handful of times; a real account reconciles continuously. A
  diagnostic reporting that every bound collection is untouched means nothing on the first, and
  ruling a cause out on that basis is the inert-probe mistake wearing a different hat. Run the
  final check against `MAILCAL_DEV_ACCOUNT=personal` as well: opening messages is read-only, and it
  is the only way to see what the person filing the bug sees.
- **A negative result is evidence only if you can show the stimulus arrived.** "The window held the
  front for 100 seconds while eight messages were delivered" was reported as proof the fault was not
  mail arriving. The messages never reached the app: the row count was 14 before and after, and the
  diagnostic said every bound collection was untouched. The probe had been inert from the start.
  Before a quiet result is allowed to rule anything out, assert that the thing being tested for
  actually happened.

## 6. Report honestly

State what was actually observed. If a check could not run (no touch screen, no harness), say so
rather than quietly narrowing the claim: a verification that hides its gaps is worse than none.
