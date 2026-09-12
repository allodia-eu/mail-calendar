#!/usr/bin/env pwsh
# The Windows client's UI test suite: assertions against the RUNNING WinUI app, driven by UI
# Automation.
#
#   ./run-ui-tests.ps1                       # every suite whose dataset is available
#   ./run-ui-tests.ps1 -Filter Unread*       # one file
#   ./run-ui-tests.ps1 -RequireHarness       # a missing harness FAILS instead of skipping
#   ./run-ui-tests.ps1 -Dataset showcase,first-run   # only the datasets named, and say so
#
# ---------------------------------------------------------------------------------------------
# WHY THIS EXISTS, AND WHAT IT IS FOR
#
# `Mailcal.Tests` is a plain net10.0 assembly and can only see code that is free of WinUI types,
# which is most of the interesting logic, deliberately, and none of the rendering. So a whole class
# of bug is invisible to it: a binding that was never wired, a property nothing assigns, a control
# that opens in the wrong state. Those shipped once already. The Windows conversation row bolded
# nothing on unread mail for as long as threading has existed, because `MailRow.Unread` was simply
# never assigned in the projection, the XAML binding was correct and had nothing to read.
#
# `cargo test` could not see it. `dotnet test` could not see it. A screenshot could, if someone
# looked at the right row and knew what they were looking for. This suite is the machine that
# looks.
#
# It is NOT a replacement for the headless suites, it is slower, it needs a desktop session, and
# it can only run on Windows. Put a rule in `Mailcal.Tests` whenever the rule can be expressed
# without WinUI. Put it here when the thing under test is what the user actually sees.
#
# WHAT CI RUNS, AND WHAT IT DOES NOT. The Windows job runs `-Dataset showcase,first-run` after its
# build, on the same runner, and a failure is red. The twelve HARNESS suites cannot run there at
# all: the harness is a Linux container and a GitHub Windows runner runs only Windows ones. So
# nothing CI checks proves a mail action survived a round trip, and the suites that do are run by
# hand (docs/background-sync.md carries the same gap for background sync). CI names them as not
# selected rather than letting them take the "the harness is not up" skip, which would silently
# reach a different set the day anything listened on that port.
#
# ---------------------------------------------------------------------------------------------
# WRITING A SUITE
#
# A `*.Tests.ps1` file defines one `$Suite` and does nothing else at load time, the runner
# dot-sources it to READ it, then launches the dataset, then invokes the cases:
#
#     $Suite = @{
#       Dataset = 'showcase'                 # 'showcase' | 'harness' | 'first-run'
#       Env = @{ MAILCAL_… = '…' }           # optional; see below
#       Prepare = { … }                    # optional; see below
#       Cases = @(
#         @{ Name = 'what must be true'; Body = { Assert-Equal 600 (…) 'why it matters' } }
#       )
#     }
#
# `Prepare` runs once, after the dataset is up and before the first case, and is for a PRECONDITION
# THE PRODUCT WILL OTHERWISE CORRECTLY REFUSE, not for setup a case could do itself. The case that
# earned it: the invitation card withholds its day preview until the calendar has actually been
# read, which is the honest answer on a store that has never synced one (docs/calendar.md §4). The
# suite had been passing only because the developer's store happened to have been synced by
# something earlier, and went red the first time it ran against a clean one. A throw here fails
# every case in the suite, which is right: the suite proved nothing.
#
# `Env` is set before the dataset launches and removed after the suite, which is the only way to
# reach a MAILCAL_* switch: the app reads them once at startup, so setting one inside a Body is
# too late, and setting one at a suite file's top level would leak into every suite that sorts
# after it (control.ps1's header documents that failure for MAILCAL_SHOWCASE, it reads as a
# passing test against the wrong dataset). It is also how a suite chooses a different harness
# transport, e.g. MAILCAL_DEV_ACCOUNT = 'stalwart-imap' for the CalDAV half.
#
# Inside a Body, every uia.ps1 primitive is in scope (Get-MailRows, Find-UiaElement, …), plus the
# assertions below. A Body asserts by THROWING; returning a value means nothing.
#
# ---------------------------------------------------------------------------------------------
# SUITES SHARE AN APP, AND WHAT THAT ASKS OF YOURS
#
# Suites declaring the same Dataset AND the same Env values run against ONE launch, in file order.
# A launch per file spent most of the run starting the app, so grouping them was worth about 270s
# when it was introduced.
#
# EVERY RUN NOW REPORTS ITS OWN NUMBERS, per suite and in the summary, split into setup and cases,
# because the two are shortened by different things and one total hides which grew: setup is app
# starts and the between-suite reset, and comes down by sharing a launch; cases are UI Automation
# tree walks, and come down by walking less. Read them before optimising anything here. The
# measurement that used to be quoted in this paragraph was two changes out of date, and a stale
# number is worse than none: it sends the next person after the half that is already cheap.
#
# Two rules follow, and the runner enforces the first for you:
#
#   1. A suite may not assume it is looking at a FRESH app. Between suites the runner puts away
#      what it knows how to put away (a ContentDialog by its CloseButton, the composer by its
#      CancelButton, a navigation away from the list) and then compares the set of AutomationIds on
#      screen against the set the launch came up with. If they do not match it RELAUNCHES. So the
#      cost of leaving something open is one app start, never a suite reading somebody else's
#      leftovers, but a suite that habitually leaves the app somewhere strange gives its group's
#      speed back. Open what you need, and prefer surfaces the runner can close.
#
#   2. A suite file's top-level variables are shared script scope, and the files COLLIDE, this
#      file has seen $Account, $Total, $Organiser, $SampleTop and $MidLuminance each defined by two
#      different suites. The runner re-loads each suite immediately before it runs so a Body always
#      closes over its OWN file's values. Read that as a warning rather than a guarantee: a variable
#      your suite uses but does not define is whatever some other file left behind. Define what you
#      read.
#
# A suite that genuinely cannot share (it needs a store in a particular state, say) gets its own
# group for free by declaring an Env value no other suite declares, which is what makes SyncHint
# and SyncHintBodies two files: a MAILCAL_* switch is read once at startup, so one launch is one
# staged hint.
#
# CHOOSING A DATASET decides whether your test proves anything (uia.ps1 header, and the
# verify-windows-ui skill §3):
#
#   showcase   two accounts, in-memory, deterministic, seeded per locale. Launched through
#              showcase.ps1, so its two safety asserts apply, the binary really carries the
#              showcase driver, and THIS process really entered showcase mode. That matters more
#              here than for a screenshot: a test that silently ran against the developer's real
#              mailbox would not just be wrong, it would be reading their mail.
#              Locale is pinned to `en`, so seeded subjects and senders are stable strings.
#   harness    one account, REAL transport, needs `scripts/dev/harness.sh up`. The only way to
#              prove something that has to survive a round trip. Skipped loudly when the harness
#              is not up, never silently, because a skipped check reads exactly like a passing
#              one in a summary line. Pass -RequireHarness to turn that into a failure.
#
# The showcase engine DOES NOT REALLY PERFORM MAIL ACTIONS. Anything destructive belongs in a
# harness suite, or it proves only that you dispatched into a void.
# ---------------------------------------------------------------------------------------------
[CmdletBinding()]
param(
  [string] $Filter = '*',
  [switch] $RequireHarness,
  # Which datasets to run, for a host that cannot serve them all. CI passes
  # `showcase,first-run`, because a GitHub Windows runner can only run WINDOWS containers and the
  # harness is a Linux one.
  #
  # A DECLARATION, not an optimisation. Leaving it out and letting the harness suites take the
  # "the harness is not up" skip reaches the same 19 suites today, and silently reaches a
  # different set the day anything starts listening on that port. What is deliberately excluded
  # is named in the summary, on its own line, next to what merely happened to be unavailable.
  #
  # No ValidateSet, and one comma-separated string is accepted as well as a list, because the two
  # ways of launching this disagree: `pwsh -File … -Dataset showcase,first-run` hands the whole
  # thing over as ONE string, while the same words inside a pwsh script are an array. ValidateSet
  # rejected the first with "the argument does not belong to the set", which reads as a wrong
  # value rather than a quoting difference and sent the reader looking at the wrong thing.
  [string[]] $Dataset = @('showcase', 'harness', 'first-run'),
  # Leave the app running afterwards, for poking at a failure by hand.
  [switch] $KeepApp
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$windows = Join-Path $here '..'

$KnownDatasets = @('showcase', 'harness', 'first-run')
$Dataset = @($Dataset | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$unknown = @($Dataset | Where-Object { $KnownDatasets -notcontains $_ })
if ($unknown.Count -gt 0) {
  throw "unknown dataset(s) '$($unknown -join ', ')'; -Dataset takes any of: $($KnownDatasets -join ', ')."
}

# Not a nicety: the whole suite drives a WinUI window over UI Automation, and both are Windows.
# Exit 0 rather than 1 so a `for each platform` loop on a Mac does not read as a red build, but
# say so out loud, because "0 tests ran" and "everything passed" print almost the same.
if (-not $IsWindows) {
  Write-Host "SKIPPED: the Windows UI suite needs a Windows desktop session (this is $([System.Runtime.InteropServices.RuntimeInformation]::OSDescription))." -ForegroundColor Yellow
  exit 0
}

. (Join-Path $windows 'uia.ps1')

# ---------------------------------------------------------------------------------------------
# Assertions. Each one throws a message that names the EXPECTED and the ACTUAL and says why the
# rule exists, a failure here is read by someone who did not write the test.
# ---------------------------------------------------------------------------------------------

function Assert-Equal {
  param([object] $Expected, [object] $Actual, [string] $Because)
  if ($Expected -ne $Actual) { throw "expected <$Expected>, got <$Actual>, $Because" }
}

function Assert-True {
  param([bool] $Condition, [string] $Because)
  if (-not $Condition) { throw "expected true, $Because" }
}

function Assert-GreaterThan {
  param([object] $Floor, [object] $Actual, [string] $Because)
  if (-not ($Actual -gt $Floor)) { throw "expected greater than <$Floor>, got <$Actual>, $Because" }
}

<#
.SYNOPSIS
The message row whose subject matches -Title (wildcards allowed), or throws naming what IS there.
.DESCRIPTION
Wildcards on purpose: a seeded subject can carry an em dash or a colon, and matching those exactly
turns a text-encoding difference into a mystifying "row not found".
#>
function Get-MailRowByTitle {
  param([Parameter(Mandatory)] [string] $Title)
  $rows = Get-MailRows
  foreach ($row in $rows) {
    if (Get-UiaTree $row | Where-Object { $_.Current.Name -like $Title }) { return $row }
  }
  $seen = ($rows | ForEach-Object { (Get-UiaTree $_ | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text } | Select-Object -First 1).Current.Name }) -join ' | '
  throw "no message row matching '$Title'. The list holds: $seen"
}

<#
.SYNOPSIS
The FontWeight of the Text element inside $Row whose content matches -Text.
.DESCRIPTION
Scoped to the row, and typed to Text, for the reason uia.ps1 trap 2 exists: a bare -Name sweep also
matches the sidebar and the reading pane, and a subject that also appears in the open message would
hand you the weight of the WRONG element, a false pass or a baffling false fail.
#>
function Get-RowTextWeight {
  param(
    [Parameter(Mandatory)] [object] $Row,
    [Parameter(Mandatory)] [string] $Text
  )
  $el = Get-UiaTree $Row |
    Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and $_.Current.Name -like $Text } |
    Select-Object -First 1
  if (-not $el) { throw "the row has no Text element matching '$Text'" }
  $weight = Get-UiaFontWeight $el
  if ($null -eq $weight) { throw "'$Text' exposes no TextPattern, so its weight cannot be read" }
  $weight
}

# ---------------------------------------------------------------------------------------------
# The dataset lifecycle, launching, readiness, and the reset that lets suites share one app.
# Its own file because every function in it is a "when is it safe to look?" decision with a
# clock in it, and each one is the answer to a specific way this suite has lied. See its header.
# ---------------------------------------------------------------------------------------------

. (Join-Path $here 'dataset.ps1')

# ---------------------------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------------------------

$files = Get-ChildItem -Path $here -Filter '*.Tests.ps1' | Where-Object { $_.BaseName -like $Filter } | Sort-Object Name
if (-not $files) { throw "no *.Tests.ps1 matched -Filter '$Filter' in $here" }

# Read every suite's DECLARATION before launching anything, so the loop below can group them: an
# app start costs ~20s against ~1-5s of assertions per suite, so one launch per FILE meant the
# runner spent most of its wall clock starting the app over and over.
#
# Suites that ask for the same dataset AND the same MAILCAL_* values want the same app, and the key
# has to carry the VALUES, not just the names: MAILCAL_FAKE_SYNC_HINT is read once at startup, so
# one launch is one staged hint, and SyncHint/SyncHintBodies stage different ones.
#
# ⚠️ READ IN A CHILD SCOPE (`& { … }`), and re-loaded below before the suite actually runs. A suite
# file's top-level variables are its own, but the files SHARE names, Appearance and TitleBar both
# define $SampleTop/$MidLuminance, SyncHint and SyncHintBodies both define $Account/$ExpectedHint,
# and a case Body closes over script scope, so it reads whatever was loaded LAST. Dot-sourcing all
# of them here would not merely fail suites; it silently passed one. Appearance's luminance case
# went green sampling TitleBar's region: a false pass, in the suite whose whole job is to notice
# that the window is painted in the wrong colour.
$plan = @()
foreach ($file in $files) {
  $declared = & { $Suite = $null; . $file.FullName; $Suite }
  if (-not $declared) { throw "$($file.Name) defines no `$Suite" }
  $envKey = ''
  if ($declared.Env) {
    $envKey = (($declared.Env.Keys | Sort-Object) | ForEach-Object { "$_=$($declared.Env[$_])" }) -join ';'
  }
  $plan += [pscustomobject]@{
    Name    = $file.BaseName
    Path    = $file.FullName
    Env     = $declared.Env
    Dataset = $declared.Dataset
    Key     = "$($declared.Dataset)|$envKey"
  }
}

# Where a failing case's screenshot goes. Emptied at the start of a run, so what is in it belongs
# to this run: a stale picture of a failure someone already fixed is worse than none, because it
# is read as evidence.
$FailureShots = Join-Path $here 'failures'
Remove-Item -Recurse -Force -LiteralPath $FailureShots -ErrorAction SilentlyContinue
$ShotsTaken = 0

<#
.SYNOPSIS
Shoots the whole desktop into uitests/failures, named for the case that just threw.
.DESCRIPTION
At the moment of the throw, which is the only moment worth photographing: the runner stops the app
on its way out and relaunches between groups, so a picture taken by anything downstream, a later
workflow step included, is of an empty desktop or of a different suite's app.

The WHOLE desktop, not the app window: a case can fail precisely because the window is gone, and
`screenshot.ps1` is the store-capture tool, which asserts about frame insets and would turn a
diagnostic into a second failure.

Never throws. A capture that fails must not replace the real failure with its own, so this
swallows everything and says so in one line.
#>
function Save-FailureShot {
  param([Parameter(Mandatory)] [string] $Suite, [Parameter(Mandatory)] [string] $Case)
  try {
    if ($script:ShotsTaken -ge 12) { return }
    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $safe = ($Case -replace '[^A-Za-z0-9]+', '-').Trim('-')
    if ($safe.Length -gt 60) { $safe = $safe.Substring(0, 60) }
    New-Item -ItemType Directory -Force -Path $script:FailureShots | Out-Null
    $path = Join-Path $script:FailureShots "$Suite-$safe.png"
    $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    try {
      $g = [System.Drawing.Graphics]::FromImage($bmp)
      try { $g.CopyFromScreen($bounds.X, $bounds.Y, 0, 0, $bmp.Size) } finally { $g.Dispose() }
      $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally { $bmp.Dispose() }
    $script:ShotsTaken++
    Write-Host "        shot: $path" -ForegroundColor DarkGray
  }
  catch {
    Write-Host "        (could not capture the screen: $($_.Exception.Message))" -ForegroundColor DarkGray
  }
}

<#
.SYNOPSIS
Throws unless the desktop is big enough to hold the window the showcase suites measure.
.DESCRIPTION
The showcase dataset pins the window to 1440x900 LOGICAL units (MainWindow.Showcase.cs), the store
capture's frame. On a smaller desktop that window is clipped, and the two ways it then lies are
both silent: `CopyFromScreen` over a region past the screen edge returns BLACK, so the luminance
suites read every theme as dark; and a NavigationView narrower than its own breakpoint switches
display mode, so the sidebar assertions measure a pane that is not the one shipped.

A GitHub Windows runner comes up at 1024x768, which fails both. It is settable
(`set-desktop-resolution.ps1`), so this throws with the number rather than letting twenty suites
fail one at a time in ways that name the toolkit instead of the screen.

Compared in LOGICAL units, which is why it goes through Get-DesktopLogicalSize rather than reading
`Screen.PrimaryScreen.Bounds` here: PowerShell 7 is DPI aware, so that property answers in physical
pixels and would pass a desktop half the size it needs. A 2880x1800 display at 200% is 1440x900,
which fits exactly, and is why showcase.ps1 grew `-NoCapture`.
#>
function Assert-DesktopFitsShowcase {
  $size = Get-DesktopLogicalSize
  if ($size[0] -ge 1440 -and $size[1] -ge 900) { return }
  throw "the desktop is $($size[0])x$($size[1]) logical pixels, and the showcase suites measure a 1440x900 window, so it would be clipped. Raise the resolution (uitests/set-desktop-resolution.ps1), or pass -Dataset harness."
}

if (($Dataset -contains 'showcase') -and ($plan.Dataset -contains 'showcase')) {
  Assert-DesktopFitsShowcase
}

$passed = 0
$failed = @()
$skipped = @()
$excluded = @()
$launches = 0
$launchSeconds = 0.0
$caseSeconds = 0.0
$wall = [Diagnostics.Stopwatch]::StartNew()

foreach ($group in ($plan | Group-Object -Property Key)) {
  $members = @($group.Group)
  # Deliberately NOT the lower-case twin of the parameter above. PowerShell variable names are
  # case-INSENSITIVE, so that name IS `-Dataset`, and a parameter declared `[string[]]` keeps its
  # type constraint: assigning this group's name to it coerces the name back into a one-element
  # ARRAY, which then reaches `Start-Dataset`'s `[string]` parameter and fails every suite in the
  # run with "cannot convert value to type System.String". Same trap as $children in uia.ps1.
  $groupDataset = $members[0].Dataset
  $groupEnv = $members[0].Env

  # Not selected. Reported apart from the skips below, because the two are different answers: this
  # one is a decision the caller made, that one is a host that could not serve the dataset.
  if ($Dataset -notcontains $groupDataset) {
    foreach ($member in $members) {
      $excluded += "$($member.Name): dataset '$groupDataset' is not in -Dataset"
    }
    continue
  }

  if ($groupDataset -eq 'harness' -and -not (Test-HarnessUp)) {
    foreach ($member in $members) {
      $reason = "$($member.Name): the harness is not up (scripts/dev/harness.sh up)"
      if ($RequireHarness) { $failed += $reason; Write-Host "FAIL  $reason" -ForegroundColor Red }
      else { $skipped += $reason; Write-Host "SKIP  $reason" -ForegroundColor Yellow }
    }
    continue
  }

  # The group's Env, applied BEFORE the launch and removed in the finally. Both halves are the
  # point: the app reads its MAILCAL_* switches once, at startup, so a variable set inside a case
  # Body is already too late; and a variable left behind would silently reconfigure every suite
  # that sorts after this one. That is the MAILCAL_SHOWCASE trap in control.ps1's header, and the
  # reason this is a declaration the runner owns rather than a `$env:` line in a suite file.
  $applied = @()
  try {
    if ($groupEnv) {
      foreach ($name in $groupEnv.Keys) {
        Set-Item "env:$name" $groupEnv[$name]
        $applied += $name
        Write-Host "    env: $name=$($groupEnv[$name])" -ForegroundColor DarkGray
      }
    }

    # $clean is the fingerprint of the app as it came up, and $null means "there is no app I am
    # entitled to reuse", the state after a launch fails, and after a suite whose setup threw.
    $clean = $null
    foreach ($entry in $members) {
      Write-Host "==> $($entry.Name)  [dataset: $groupDataset]" -ForegroundColor Cyan
      # Load the suite HERE, into script scope, so its own top-level variables are the ones its
      # case bodies close over, see the warning on the grouping pass above.
      $Suite = $null
      . $entry.Path
      # Timed in two halves, because they are shortened by different things and a single total
      # hides which one grew: setup is app starts and the between-suite reset, and comes down by
      # sharing a launch; cases are UI Automation tree walks, and come down by walking less.
      $setup = [Diagnostics.Stopwatch]::StartNew()
      try {
        if ($null -eq $clean) {
          Start-Dataset $groupDataset
          $launches++
          $clean = Get-SurfaceFingerprint
        }
        elseif (-not (Reset-AppSurface $clean)) {
          Write-Host '    the suite before left a surface this runner cannot put away; relaunching' -ForegroundColor DarkGray
          Start-Dataset $groupDataset
          $launches++
          $clean = Get-SurfaceFingerprint
        }
        if ($Suite.Prepare) { & $Suite.Prepare }
      }
      catch {
        # The dataset never came up, or the precondition the suite needs was refused. Every case in
        # it would now be measuring nothing, so fail them ALL by name, a suite that silently
        # vanishes from the summary reads exactly like one that passed.
        Write-Host "  FAIL  (setup) $($_.Exception.Message)" -ForegroundColor Red
        foreach ($case in $Suite.Cases) {
          $failed += "$($entry.Name) / $($case.Name): setup failed, $($_.Exception.Message)"
        }
        $launchSeconds += $setup.Elapsed.TotalSeconds
        $clean = $null
        continue
      }
      $setup.Stop()
      $launchSeconds += $setup.Elapsed.TotalSeconds

      $cases = [Diagnostics.Stopwatch]::StartNew()
      foreach ($case in $Suite.Cases) {
        try {
          & $case.Body
          $passed++
          Write-Host "  PASS  $($case.Name)" -ForegroundColor Green
        }
        catch {
          $failed += "$($entry.Name) / $($case.Name): $($_.Exception.Message)"
          Write-Host "  FAIL  $($case.Name)" -ForegroundColor Red
          Write-Host "        $($_.Exception.Message)" -ForegroundColor Red
          Save-FailureShot -Suite $entry.Name -Case $case.Name
        }
      }
      $cases.Stop()
      $caseSeconds += $cases.Elapsed.TotalSeconds
      Write-Host ("      {0:N1}s setup + {1:N1}s in {2} case(s)" -f
        $setup.Elapsed.TotalSeconds, $cases.Elapsed.TotalSeconds, $Suite.Cases.Count) -ForegroundColor DarkGray
    }
  }
  finally {
    foreach ($name in $applied) { Remove-Item "env:$name" -ErrorAction SilentlyContinue }
  }
}

if (-not $KeepApp) { Get-Process Mailcal -ErrorAction SilentlyContinue | Stop-Process -Force }

$ran = $plan.Count - $excluded.Count - $skipped.Count

Write-Host ''
Write-Host "$passed passed, $($failed.Count) failed, $($skipped.Count) skipped, $($excluded.Count) not selected"
Write-Host ("$ran suite(s) ran in {0:N0}s: {1:N0}s setup over $launches app launch(es), {2:N0}s in cases" -f
  $wall.Elapsed.TotalSeconds, $launchSeconds, $caseSeconds)
foreach ($s in $skipped) { Write-Host "  SKIPPED: $s" -ForegroundColor Yellow }
# One line, not one per suite: an excluded dataset is a whole block of the suite the caller asked
# not to run, and printing nineteen identical reasons buries the skips above that are not that.
if ($excluded.Count -gt 0) {
  Write-Host ("  NOT SELECTED: $($excluded.Count) suite(s), -Dataset is " + ($Dataset -join ',')) -ForegroundColor Yellow
}
if ($failed.Count -gt 0) {
  foreach ($f in $failed) { Write-Host "  FAILED:  $f" -ForegroundColor Red }
  exit 1
}
# A run that measured nothing is a failure, not a pass. On this host the reachable ways to get here
# are a -Filter matching only suites whose dataset was excluded, and a -Dataset naming only the
# harness on a machine that has none: both leave a green step that checked nothing, which is the
# single most expensive shape a CI gate can take. The non-Windows case above exits 0 deliberately,
# and says SKIPPED out loud, because there the whole suite is inapplicable rather than empty.
if ($ran -le 0) {
  Write-Host 'FAILED: no suite ran at all, so this run proves nothing.' -ForegroundColor Red
  exit 1
}
exit 0
