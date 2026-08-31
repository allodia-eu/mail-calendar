#!/usr/bin/env pwsh
# The dataset lifecycle for the Windows UI suite: bringing an app up under a named dataset, deciding
# when it is genuinely READY, and getting it back to a clean surface so the next suite can reuse it.
#
# Dot-sourced by run-ui-tests.ps1, which owns the assertions and the run loop. Split out because
# these are the parts with a clock in them, every function here is a "when is it safe to look?"
# decision, and each one is the answer to a specific way this suite has lied:
#
#   Wait-DatasetReady      three sequential waits became one poll, because the first of them burned
#                          its whole timeout on every showcase launch probing for a screen that
#                          cannot appear there.
#   Wait-CalendarSynced    mail rows land before the calendar does, so a suite that opened a meeting
#                          the moment rows appeared read the CACHED calendar and reported a missing
#                          organiser as a projection bug.
#   Get-AppSessionLog      the log rotates at a size cap, so a byte offset taken before the launch
#                          can outlive the bytes it points at.
#   Reset-AppSurface       what makes one launch safely serve several suites, and, when it cannot,
#                          says so and lets the caller relaunch instead of guessing.
#
# Nothing here asserts. A failure is a throw that names which stage was reached, because "no window",
# "no message list" and "no rows" have three different causes and one message could only name one.

# clients/windows, where showcase.ps1 and control.ps1 live. Resolved from THIS file rather than
# inherited from whoever dot-sourced it, so the dependency is visible and the file stands alone.
$ClientDir = Join-Path $PSScriptRoot '..'

# ---------------------------------------------------------------------------------------------
# Datasets. Start-Dataset returns only once the app is up, the message list has rows, and, on the
# harness, which fetches over the wire, the calendar has finished its first refresh from the
# server. Every one of those is polled, never slept (uia.ps1 header).
# ---------------------------------------------------------------------------------------------

function Test-HarnessUp {
  try {
    $client = [System.Net.Sockets.TcpClient]::new()
    $ok = $client.ConnectAsync('127.0.0.1', 28080).Wait(1500)
    $client.Dispose()
    return $ok
  }
  catch { return $false }
}

<#
.SYNOPSIS
Block until the dataset is up: past the first-run question, message list drawn, rows in it.
.DESCRIPTION
ONE tree walk per turn answers all three questions. They used to be three sequential waits, and the
first of them cost its full timeout on every showcase launch, the welcome screen cannot appear
there (a build with no preferences store reports the question settled by construction), so probing
for it separately could only ever run out the clock.

THE FIRST-RUN QUESTION. A store that has never been asked puts the welcome screen up ahead of
everything (`AnalyticsConsent.asked`), and it has no message list, so every harness suite fails
with "the message list never appeared", naming the wrong thing entirely. That is not hypothetical:
`harness.sh reset` clears the client dev stores, precisely so a re-bootstrapped server cannot serve
its reused ids against the last generation's cached bodies, and the very next run lands here.

It answers with **Get started**, never "Share usage statistics": a test run must not opt this
machine into analytics, and off is the documented default (docs/analytics.md). By automation id,
not by label, the app's language on a harness run is whatever the developer prefers.

The throw names the furthest stage reached: "no window", "no list" and "no rows" have three
different causes, and the message it replaces could only ever name the second.
#>
function Wait-DatasetReady {
  param([Parameter(Mandatory)] [string] $Dataset, [int] $TimeoutSec = 90)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  $settled = $false
  $stage = 'no app window'
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    # The window may not be up yet, which Get-UiaTree throws on, that is a "not yet", not a failure.
    $tree = @()
    try { $tree = @(Get-UiaTree) } catch { }
    if ($tree.Count -gt 0) {
      if ($stage -eq 'no app window') { $stage = 'a window, but the message list never appeared' }
      if (-not $settled) {
        $welcome = $tree | Where-Object { $_.Current.AutomationId -eq 'WelcomeGetStarted' } | Select-Object -First 1
        if ($welcome) {
          Write-Host '    first run: settling the usage-statistics question (declining)' -ForegroundColor DarkGray
          Invoke-UiaElement $welcome
          $settled = $true
          continue
        }
      }
      # `first-run` settles on the account-setup form, not on a list: its namespace is empty, so a
      # message list is the one thing that can never appear there. It still goes through the
      # welcome question above, that screen comes first (docs/onboarding.md).
      #
      # ON SCREEN, and that is the whole of it. MainWindow builds the welcome view and the setup
      # view together, so `DetectEmail` is in the tree from the first walk, behind the welcome
      # screen, before anyone has answered it. Matching the bare id therefore returns while the
      # welcome screen is still up, and the suite then measures a screen with no card on it and
      # reports the card missing. Measured: it passed twice and failed on the third run, which is
      # what a race looks like from the outside.
      if ($Dataset -eq 'first-run') {
        $stage = 'a window, but the account-setup form never came to the front'
        if ($tree | Where-Object {
            $_.Current.AutomationId -eq 'DetectEmail' -and -not $_.Current.IsOffscreen
          }) { return }
        Start-Sleep -Milliseconds 200
        continue
      }
      $list = $tree | Where-Object { $_.Current.AutomationId -eq 'RowsList' } | Select-Object -First 1
      if ($list) {
        $stage = 'the message list, but no rows ever arrived in it'
        # Rows arrive after the list does; the harness fetches them over the wire.
        if (@(Find-UiaElements -Type 'ListItem' -Root $list).Count -gt 0) { return }
      }
    }
    Start-Sleep -Milliseconds 200
  }
  throw "dataset '$Dataset' was not ready within ${TimeoutSec}s, got as far as: $stage. If the store was just cleared (scripts/dev/harness.sh reset does that), the app may be sitting on a screen this runner does not know how to get past, run it by hand once and look."
}

<#
.SYNOPSIS
The set of AutomationIds on screen, as one comparable string.
.DESCRIPTION
This IS the definition of "clean" used below: whatever the tree holds right after a launch. A suite
that leaves a dialog, a composer or the calendar up changes the set, so comparing against the
launch-time value catches leftovers without anyone having to enumerate what each suite opens.
#>
function Get-SurfaceFingerprint {
  param([object[]] $Tree)
  if (-not $Tree) { $Tree = @(Get-UiaTree) }
  # ON-SCREEN only, and that is the whole difficulty. A NavigationView keeps the page you navigated
  # away from in the tree, so a fingerprint taken over everything never matches again once a suite
  # has visited the calendar: the runner relaunches for that suite and every suite after it, and
  # hands back most of the saving. IsOffscreen is what separates "still built" from "still shown".
  ($Tree | Where-Object { -not $_.Current.IsOffscreen -and $_.Current.AutomationId } |
    ForEach-Object { $_.Current.AutomationId } | Sort-Object -Unique) -join ','
}

<#
.SYNOPSIS
Put away whatever the last suite left open; $true when the app is back to its launch state.
.DESCRIPTION
Returns $false rather than trying harder, and the caller then relaunches, so the cost of not
knowing how to close something is one app start, never a suite measuring somebody else's leftovers.
#>
function Reset-AppSurface {
  param([Parameter(Mandatory)] [string] $Clean)
  $button = [System.Windows.Automation.ControlType]::Button
  for ($attempt = 0; $attempt -lt 3; $attempt++) {
    # ONE walk answers every question below. Each Find-UiaElement would be another full crossing of
    # the tree, and this runs between every pair of suites, five walks an attempt was costing more
    # than the relaunch it was trying to avoid.
    $tree = @(Get-UiaTree)
    if ((Get-SurfaceFingerprint $tree) -eq $Clean) { return $true }
    $shown = @($tree | Where-Object { -not $_.Current.IsOffscreen })

    # Every ContentDialog carries CloseButton and the composer carries CancelButton; between them
    # they close everything the suites here open over the list.
    $acted = $false
    foreach ($id in 'CancelButton', 'CloseButton') {
      $close = $shown |
        Where-Object { $_.Current.AutomationId -eq $id -and $_.Current.ControlType -eq $button } |
        Select-Object -First 1
      if ($close) { Invoke-UiaElement $close; Start-Sleep -Milliseconds 250; $acted = $true }
    }

    # A suite that navigated away (the calendar) took the message list with it. The unified inbox is
    # the FIRST item in the navigation pane, positional on purpose: it is the one sidebar entry
    # carrying no AutomationId, and its label is whatever language the app is running in.
    if (-not ($shown | Where-Object { $_.Current.AutomationId -eq 'RowsList' })) {
      $nav = $tree | Where-Object { $_.Current.AutomationId -eq 'Nav' } | Select-Object -First 1
      if ($nav) {
        # NOT $home: PowerShell variable names are case-insensitive and $HOME is a read-only
        # automatic, so assigning it throws, and the throw surfaces as the NEXT suite failing at
        # setup, naming nothing that points back here. Same trap as $children in uia.ps1.
        $unified = Find-UiaElements -Type 'ListItem' -Root $nav | Select-Object -First 1
        if ($unified) { Invoke-UiaElement $unified; Start-Sleep -Milliseconds 500; $acted = $true }
      }
    }

    # Nothing left to try. The commonest dirty state is a message open in the reading pane, which
    # has no close affordance at all, so retrying is a tree walk a second and a third time to reach
    # the same answer, and the caller is going to relaunch either way. Give up while it is cheap.
    if (-not $acted) { return $false }
  }
  (Get-SurfaceFingerprint) -eq $Clean
}

# Mirrors MailboxModel.DataDir + Log.Init, the same path showcase.ps1 reads.
$AppLogPath = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\logs\app.log'

<#
.SYNOPSIS
Block until THIS launch has finished its first calendar refresh from the server.
.DESCRIPTION
Polled off the core's own log, because the calendar has no on-screen "done" and every word the
shell could show is localized, a harness run comes up in whatever language the developer prefers,
so a marker read off the UI would make the suite pass or fail by whose machine it ran on. The core
writes this line in every locale.

The distinction that matters is between the two calendar rebuilds a launch performs. The first is
served from the local cache within ~300ms; `refresh_calendar` is the one that has been to the
server, ~4s in. Waiting for the first proves nothing, which is precisely the bug this exists to
stop.
#>
function Wait-CalendarSynced {
  param([Parameter(Mandatory)] [datetime] $Since, [int] $TimeoutSec = 60)
  $marker = 'refresh_calendar:'
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    if ((Get-AppSessionLog $Since).IndexOf($marker, [StringComparison]::Ordinal) -ge 0) { return }
    Start-Sleep -Milliseconds 200
  }
  throw "the calendar never finished its first refresh within ${TimeoutSec}s (no '$marker' in this launch's session in $AppLogPath). The harness may be up but not serving calendars, scripts/dev/harness.sh up."
}

<#
.SYNOPSIS
The log lines belonging to the session that started at or after $Since, or '' if none has yet.
.DESCRIPTION
Anchored on the last `--- session start` banner and its TIMESTAMP rather than a byte offset taken
before the launch, for the two reasons showcase.ps1's Get-SessionLog spells out: the log rotates at
a size cap, so an offset can outlive the bytes it addresses; and without the timestamp, an app that
never started leaves the previous session's banner answering for a launch that never happened.
#>
function Get-AppSessionLog {
  param([Parameter(Mandatory)] [datetime] $Since)
  if (-not (Test-Path $AppLogPath)) { return '' }
  $stream = [IO.File]::Open($AppLogPath, 'Open', 'Read', 'ReadWrite')
  try { $text = (New-Object IO.StreamReader($stream, [Text.Encoding]::UTF8)).ReadToEnd() }
  finally { $stream.Dispose() }

  $banners = [regex]::Matches($text, '(?m)^(?<at>\S+ \S+ \S+) \[info\] --- session start')
  if ($banners.Count -eq 0) { return '' }
  $last = $banners[$banners.Count - 1]
  $at = [datetimeoffset]::MinValue
  if (-not [datetimeoffset]::TryParse($last.Groups['at'].Value, [ref] $at)) { return '' }
  if ($at -lt ([datetimeoffset] $Since).AddSeconds(-1)) { return '' }
  $text.Substring($last.Index)
}

function Start-Dataset {
  param([Parameter(Mandatory)] [string] $Dataset)
  $launchedAt = Get-Date
  switch ($Dataset) {
    'showcase' {
      # Through showcase.ps1, never by setting the env var here: it is the thing that proves the
      # binary carries the showcase driver and that THIS process entered showcase mode. -NoCapture
      # keeps both of those asserts and skips only the shutter, the capture was a side effect, and
      # it cannot succeed on a 2880x1800 display at 200% (the pinned frame is the whole screen, so
      # the window overhangs it), which was blocking every suite on such a host.
      #
      # A suite's MAILCAL_APPEARANCE has to be handed over as a PARAMETER. showcase.ps1 pins the
      # appearance rather than inheriting one, that is what stops a store capture coming out in
      # whatever the developer's desktop is set to, so it overwrites the ambient value with its
      # own default. Left to $Suite.Env alone, the runner prints the pin it asked for and the app
      # comes up in the default: a suite that reads as pinned while measuring nothing.
      #
      # SettleSeconds = 0 because showcase.ps1's blind post-launch sleep is for the SHUTTER, and
      # this path takes no picture: Assert-ShowcaseRunning polls for its marker and Wait-DatasetReady
      # polls for the list. Left at the default it was seven seconds of dead time per launch.
      $showcase = @{ Locale = 'en'; Screen = 'list'; NoCapture = $true; SettleSeconds = 0 }
      if ($env:MAILCAL_APPEARANCE) { $showcase.Appearance = $env:MAILCAL_APPEARANCE }
      & (Join-Path $ClientDir 'showcase.ps1') @showcase | Out-Null
    }
    'harness' {
      & (Join-Path $ClientDir 'control.ps1') home | Out-Null
    }
    'first-run' {
      # The one screen a person sees ONCE. Its namespace is wiped first, because a first run is
      # defined by there being nothing in it: an account or a settled welcome question left by the
      # last run makes the screen unreachable, and the suite would then assert against whatever it
      # landed on instead. control.ps1 launches it, the same as the harness, MAILCAL_DEV_ACCOUNT
      # comes from the suite's Env and it honours an inherited one.
      $store = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\dev-first-run'
      Remove-Item -Recurse -Force -LiteralPath $store -ErrorAction SilentlyContinue
      if (Test-Path -LiteralPath $store) {
        throw "the first-run store at $store could not be cleared, so the app would not open on a first run, close any running Mailcal.exe and retry."
      }
      & (Join-Path $ClientDir 'control.ps1') home | Out-Null
    }
    default { throw "unknown dataset '$Dataset' (showcase | harness | first-run)" }
  }
  Wait-DatasetReady $Dataset
  # The message list is not the whole dataset. Mail rows land first and the calendar is still
  # arriving behind them, so a suite that opens a meeting the moment rows appear reads the CACHED
  # calendar: the seeded meeting's organiser is missing while its other two attendees are already
  # there, which reads as a projection bug rather than a race. Only the harness fetches over the
  # wire; the showcase engine is in memory and has nothing to wait for.
  if ($Dataset -eq 'harness') { Wait-CalendarSynced $launchedAt }
}
