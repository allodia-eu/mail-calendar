---
name: verify-windows-ui
description: Drive and ASSERT on the running WinUI (Windows) client from a script: UI Automation via clients/windows/uia.ps1, plus the MAILCAL_* launch hooks for what UIA cannot reach (gestures, context menus). Use when verifying that a Windows UI change actually works, not just that it compiles. Covers the traps that produce silent FALSE PASSES, and which dataset proves what.
---

# verify-windows-ui: prove a Windows UI change actually works

Windows-only (needs the built client and a desktop session). To *boot* a client, see **debug-app**;
this skill is about **asserting** on it afterwards.

The client is driven by **UI Automation**, never by synthetic mouse input. The primitives live in
[`clients/windows/uia.ps1`](../../../clients/windows/uia.ps1): **dot-source it, don't reimplement
it.** Its header documents traps that a hand-rolled walk will hit, and three of them hand you a
**green assertion for something that is not on screen**. That is worse than a crash, because you
believe it and move on.

## 0. Prerequisites

```powershell
./clients/windows/build-and-run.ps1 -NoRun     # build first; assertions run against the built exe
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
  Dataset = 'showcase'          # or 'harness': §3 decides which proves your point
  Env = @{ MAILCAL_… = '…' }    # optional; set before launch, removed after the suite
  Cases = @(
    @{ Name = 'what must be true'; Body = { Assert-Equal 600 (Get-RowTextWeight -Row $r -Text 'x') 'why' } }
  )
}
```

**`Env` is how a suite reaches a `MAILCAL_*` switch**, and both halves of the runner's handling
matter: the app reads them once at startup, so a `$env:` line inside a Body is already too late,
and one set at a suite file's top level leaks into every suite that sorts after it (§5's
`MAILCAL_SHOWCASE` trap, which reads as a passing test against the wrong dataset). It is also how a
suite picks a different harness transport: `MAILCAL_DEV_ACCOUNT = 'stalwart-imap'` for the CalDAV
half.

**When the state you need is one the harness cannot produce, fake the *input*, never the surface.**
`InvitationReplyPrompt.Tests.ps1` is the worked example: no server here ever reports a failed iTIP
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
```

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

UIA drives *controls*. It has no notion of a gesture, so for anything gesture-shaped you need one of
the two tools below. Pick by what you are actually trying to prove.

### Gestures: inject real touch (`touch.ps1`)

**This skill used to say a WinUI gesture "cannot be synthesized". That was wrong** (corrected
2026-07-13, verified against a packaged WinUI 3 app). Win32's `InitializeTouchInjection` /
`InjectTouchInput` inject at the **pointer-device** level, so the OS delivers them as genuine touch
and they drive the real gesture pipeline (`SwipeControl`, `ScrollView`, and the calendar grid's own
pointer owner) from an ordinary **unelevated, unpackaged** process. No capability, no package
identity, no elevation, no Store approval.

(The *WinRT* `Windows.UI.Input.Preview.Injection.InputInjector` **does** need the
`inputInjectionBrokered` restricted capability *and* package identity, which the unpackaged dev loop
has not got. That is almost certainly where the "impossible" came from.)

```powershell
. ./clients/windows/touch.ps1
Initialize-Touch
$w = Get-MailcalBounds                                   # foregrounds the app; physical pixels
Invoke-TouchFlick -FromX ($w.Right-300) -ToX ($w.Left+300) -Y $w.MidY    # turn the calendar week
Invoke-TouchPinch -CenterX $w.MidX -CenterY $w.MidY -FromSpread 150 -ToSpread 700 -AngleDeg 45
```

Its header documents the five traps (each an afternoon): `POINTER_FLAG_NEW` on the DOWN frame → error
87; coordinates are **physical** screen pixels, so the injector must be DPI-aware; the UP frame must
be at the same point as the last UPDATE; frames must be paced; and **it is a real finger**: it goes
to whatever window is under the point, so assert what is on screen *before* injecting, never after.

**A synthetic gesture cannot test the bug that matters.** It cannot land a flick one frame into the
previous gesture's *animation*, and that race is where the calendar's swallowed swipe lived, so it
tests the case that already worked (`docs/calendar.md` §9). Put that race in a **unit** test that owns
the clock (`Mailcal.Tests/CalendarFlickTests.cs`). Use both.

### The system save/open picker: reachable, but only by keyboard

`FileSavePicker` (e.g. Settings → Diagnostics → *Export log…*) opens the classic Save As dialog,
and three things about it defeat the obvious approaches (each cost a retry, 2026-07-16):

- It is a **child** window (class `#32770`) of the app window, **never** a top-level window, so
  polling `RootElement`'s children for a new window finds nothing. Find it as
  `(Get-MailcalWindow).FindFirst(Children, ClassName == '#32770')`.
- Its controls expose a **degenerate UIA tree**: the filename box (`#1001` under
  `#FileNameControlHost`) and the Save button (`#1`) are patternless Panes, with no `ValuePattern`, no
  `InvokePattern`, and `SetFocus()` throws. Raw `TreeScope.Descendants` also under-walks it
  (trap 1 all over again).
- What **works** is the keyboard: `SetForegroundWindow` on the dialog's `NativeWindowHandle`, then
  plain keystrokes, since the filename box has default focus, so `SendKeys` `^a`, the full target path,
  `{ENTER}` saves. Assert on the **written file** afterwards (bytes, not existence), never on the
  dialog.

### Context flyouts, file pickers, video: Microsoft's `winapp` CLI

**This skill used to say context flyouts were "genuinely out of reach". That was wrong** (corrected
2026-07-31). Microsoft's `winapp` CLI (`winget install Microsoft.WinAppCLI`, and the skill suite in
the `microsoft/win-dev-skills` plugin) opens the mail row's flyout on the first try:

```powershell
winapp ui inspect "RowsList" -a $AppPid -d 3          # discover the row slugs
winapp ui click "itm-…" -a $AppPid --right            # opens it: PopupHost + "Archive conversation"
winapp ui invoke "mnu-archiveconversa-…" -a $AppPid
```

The old diagnosis (the flyout hangs off an inner `Grid`, so `ContextRequested` never reaches it) was
simply not the obstacle. `winapp` is **DPI-aware** and clicks the real physical point, which is the
same reason "the mouse generally" was written off here. Reach for it for the three things `uia.ps1`
still can't do: **context flyouts**, the **system file picker** (`-w <HWND>` + `set-value
"FileNameControlHost"`, likely simpler than the keyboard dance above, untested here), and
**`winapp ui record`** for an H.264 repro clip.

Two traps before you lean on it:

- **The slugs are per-run hashes** (`itm-allodiamailcalv-f83c`). Discover-then-click in one script;
  never hardcode one in a suite.
- **`winapp ui inspect --json` nests its tree under `windows[].elements[]`.** Reading `.elements` at
  the root yields `$null`, and `@($null | Where-Object …).Count -eq 0` then reports a confident
  PASS having examined nothing. Microsoft's own `winui-ui-testing` skill ships this bug in its
  accessibility-audit template; verified against `winapp` 0.5.0.

**Keep `uia.ps1` as the default.** `winapp` exposes exactly 10 UIA properties and no text
attributes, so it **cannot read font weight**: `Get-UiaFontWeight` is what
`uitests/UnreadEmphasis.Tests.ps1` asserts on, and that is the suite guarding the never-assigned
`MailRow.Unread` binding. It also selects only by name/automationId, so
`winapp ui search "CalendarSurface"` finds **0 matches** where our `ClassName` walk finds the
surface and 48 nodes under it. Use it as a second instrument, not a replacement.

**Don't adopt the rest of that plugin's workflow.** Its `winui-dev-workflow` assumes `dotnet new
winui-mvvm` and forbids `<WindowsPackageType>None`, which is the opposite of this client (unpackaged
+ self-contained for the dev loop, Rust cdylib built first); its `winui-code-review` MVVM section
assumes `CommunityToolkit.Mvvm` ViewModels, which this client does not use (it renders immutable
snapshots from the core), and its theming rule flags `ReadingView.xaml`'s deliberate literal white
message canvas. Its `winui-packaging` is dev-cert grade, not `.msixupload`.

**`Microsoft.WindowsAppSDK.Analyzers` is worth revisiting, but not yet** (checked 2026-07-31). The
WUI0xxx–WUI4xxx rules are a real build-time gate we have no equivalent of: `x:Bind` without `Mode`,
missing AutomationId, UWP-era APIs. But it is **not on nuget.org**: the only copy is a loose 49 KB
DLL inside the plugin's own version-pinned cache directory, so wiring it means either a
machine-specific path (dead on CI and on the Mac) or vendoring an undocumented binary that a plugin
update moves. Adopt it when Microsoft ships the package.

### Still genuinely out of reach

- **The mouse from `uia.ps1` itself.** UIA drives patterns, not pointers; use `winapp` above.

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
check that what is announced is where it is drawn.

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
  `eu.allodia.mailcal:dev:account:<id>` (chunks `:1`, `:2`, … if any). Use a *second* seed account
  (`bob@test.local` / `harness-bob-pw`) so the run's own canned account stays untouched. (If you ever
  add through the form in a **non-dev** run, that DOES hit the real `eu.allodia.mailcal:` store and
  its index co-mingles real accounts; there, remove surgically, never nuke the index.)
- **Preference files are dev-isolated too.** Under `MAILCAL_DEV_ACCOUNT` the one-line preference
  files (language, window placement, pane width, the Diagnostics log level) read/write inside the
  dev store subdir (`AppPaths.PrefsDir`), so resizing the window or flipping the Settings →
  Diagnostics DEBUG toggle in a test never changes the developer's real app's next launch. The
  rotating `app.log` deliberately stays **shared**: one file diagnoses whatever ran last, so a
  test that asserts on log content needs no dev-specific path, but must tolerate real-run lines.

## 6. Report honestly

State what was actually observed. If a check could not run (no touch screen, no harness), say so
rather than quietly narrowing the claim: a verification that hides its gaps is worse than none.
