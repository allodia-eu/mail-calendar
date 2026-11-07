#!/usr/bin/env pwsh
# The Windows client's UI suites: assertions against the RUNNING WinUI app, made from inside it
# through its test agent. The suites live in clients/windows/Mailcal.UiTests, whose README says how
# they work and how to write one; this is the one command that runs them by dataset.
#
#   ./run-ui-tests.ps1                                 # every suite whose dataset this host can serve
#   ./run-ui-tests.ps1 -Filter Unread*                 # one suite, by its name (UnreadEmphasisTests)
#   ./run-ui-tests.ps1 -RequireHarness                 # a missing harness FAILS instead of skipping
#   ./run-ui-tests.ps1 -Dataset showcase,first-run     # only the datasets named, and say so
#
# WHY THIS EXISTS. Mailcal.Tests is a plain net10.0 assembly and sees only code free of WinUI types,
# so an unwired binding, a property nothing assigns and a control opening in the wrong state are
# all invisible to it. The Windows conversation row bolded nothing on unread mail for as long as
# threading existed, because MailRow.Unread was never assigned; neither cargo test nor dotnet test
# could see it. These suites are the machine that looks. Put a rule in Mailcal.Tests whenever it can
# be stated without WinUI, and here when the thing under test is what the user sees.
#
# A DATASET decides whether a suite proves anything:
#
#   showcase   two accounts, in memory, deterministic, seeded per locale and pinned to `en`. The
#              launch asserts the binary carries the showcase driver and that this process entered
#              showcase mode, because one that silently ran against the developer's real mailbox
#              would be reading their mail. It performs no real mail action.
#   first-run  the real core over an emptied store: the screens a person sees once.
#   harness    the local Stalwart server (scripts/dev/harness.sh up): the only way to prove a mail
#              action survived a round trip. A case skips, saying so, when the harness is down;
#              -RequireHarness makes that a failure.
#
# WHAT CI RUNS. The Windows job runs `-Dataset showcase,first-run` after its build. The harness is a
# Linux container and a GitHub Windows runner runs only Windows ones, so the harness suites run
# nowhere but a developer's machine, and CI names them as not selected rather than letting them
# take the harness-is-down skip, which would silently reach a different set the day anything
# listened on that port.
[CmdletBinding()]
param(
  [string] $Filter = '*',
  [switch] $RequireHarness,
  # The datasets to run, for a host that cannot serve them all: a declaration, so what is excluded is
  # named in the summary apart from what was merely unavailable. One comma-separated string is taken
  # as well as a list, because `pwsh -File … -Dataset a,b` hands the whole thing over as one string.
  [string[]] $Dataset = @('showcase', 'harness', 'first-run'),
  # Leave the app running afterwards, for poking at a failure by hand.
  [switch] $KeepApp
)
$ErrorActionPreference = 'Stop'
$project = Join-Path $PSScriptRoot '..\Mailcal.UiTests'
. (Join-Path $PSScriptRoot 'agent-suites.ps1')

$KnownDatasets = @('showcase', 'harness', 'first-run')
$Dataset = @($Dataset | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$unknown = @($Dataset | Where-Object { $KnownDatasets -notcontains $_ })
if ($unknown.Count -gt 0) {
  throw "unknown dataset(s) '$($unknown -join ', ')'; -Dataset takes any of: $($KnownDatasets -join ', ')."
}

# The suites drive a WinUI window, which is Windows. Exit 0 so a loop over every platform on a Mac
# does not read as a red build, but say so: "0 tests ran" and "everything passed" print alike.
if (-not $IsWindows) {
  Write-Host "SKIPPED: the Windows UI suites need a Windows desktop session (this is $([System.Runtime.InteropServices.RuntimeInformation]::OSDescription))." -ForegroundColor Yellow
  exit 0
}

# Each suite is a `<Name>Tests` class, matched by -Filter as `<Name>.Tests`, and declares its
# dataset as a trait. Both are read from the suites' own listing of what they compiled rather than
# from their source, and a suite that declares none is refused rather than never run.
$build = & dotnet build $project --nologo -v q 2>&1
if ($LASTEXITCODE -ne 0) {
  $build | ForEach-Object { Write-Host $_ }
  throw "the UI suites did not build"
}
$listing = & dotnet run --project $project --no-build -- -list full/json 2>&1
if ($LASTEXITCODE -ne 0) { throw "the UI suites could not list their cases: $($listing -join ' ')" }
$suites = @(ConvertFrom-AgentTestList -Listing ($listing -join "`n") | Where-Object { $_.Name -like $Filter } | Sort-Object Name)
if (-not $suites) { throw "no suite in $project matched -Filter '$Filter'" }

$excluded = @($suites | Where-Object { $Dataset -notcontains $_.Dataset })
$chosen = @($suites | Where-Object { $Dataset -contains $_.Dataset })
$passed = @()
$failed = @()
$skipped = @()
$wall = [Diagnostics.Stopwatch]::StartNew()

if ($chosen) {
  Write-Host "==> $(($chosen.Name) -join ', ')" -ForegroundColor Cyan
  # The suites are a program of their own (xUnit v3), run directly rather than through a test host.
  # The verbose reporter prints each case as it starts and as it ends, so a run cut short, by a hang
  # or a job's time limit, shows the case it was in; the summary below is read from the report.
  $report = Join-Path ([IO.Path]::GetTempPath()) "mailcal-uitests-$PID.xml"
  $arguments = @('-noLogo', '-reporter', 'verbose', '-result-xml', $report) +
    @($chosen | ForEach-Object { '-class', "Allodia.Mailcal.UiTests.Suites.$($_.Class)" })
  # MAILCAL_UITESTS_REQUIRE_HARNESS turns a missing harness from a skip into a failure, as
  # -RequireHarness promises.
  if ($RequireHarness) { $env:MAILCAL_UITESTS_REQUIRE_HARNESS = '1' }
  # What a failing case leaves is named after the case, so one left by an earlier run would read as
  # this run's; so would an earlier run's report.
  Remove-Item (Join-Path $project 'failures') -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item $report -ErrorAction SilentlyContinue
  try {
    & dotnet run --project $project --no-build -- @arguments 2>&1 | ForEach-Object { "$_" } | Tee-Object -Variable output | ForEach-Object { Write-Host $_ }
    $exit = $LASTEXITCODE
  }
  finally {
    Remove-Item env:MAILCAL_UITESTS_REQUIRE_HARNESS -ErrorAction SilentlyContinue
  }
  if (Test-Path $report) {
    # A report cut short is a failure to name, not a reason to skip the summary and leave the app up.
    try {
      $result = ConvertFrom-AgentTestReport -Report ([xml] (Get-Content -Raw $report))
      $passed = $result.Passed
      $failed = $result.Failed
      $skipped = $result.Skipped
    }
    catch {
      $failed += "the suites' report could not be read: $($_.Exception.Message)"
    }
    Remove-Item $report -ErrorAction SilentlyContinue
  }
  # A run that wrote no report is a failure that names no case.
  if ($exit -ne 0 -and $failed.Count -eq 0) {
    $failed += "the suites exited $exit without a failing case; their output ends: $((@($output) | Select-Object -Last 5) -join ' | ')"
  }
}

# Never fatal: a crashed app that Windows Error Reporting still holds refuses to be stopped, and the
# summary below is what says what the run found.
if (-not $KeepApp) { Get-Process Mailcal -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue }

Write-Host ''
Write-Host ("$($passed.Count) passed, $($failed.Count) failed, $($skipped.Count) skipped, $($excluded.Count) suite(s) not selected, in {0:N0}s" -f $wall.Elapsed.TotalSeconds)
foreach ($s in $skipped) { Write-Host "  SKIPPED: $s" -ForegroundColor Yellow }
# One line: an excluded dataset is a block the caller asked not to run, and a line each would bury
# the skips above, which are not that.
if ($excluded.Count -gt 0) {
  Write-Host ("  NOT SELECTED: $($excluded.Count) suite(s), -Dataset is " + ($Dataset -join ',')) -ForegroundColor Yellow
}
if ($failed.Count -gt 0) {
  foreach ($f in $failed) { Write-Host "  FAILED:  $f" -ForegroundColor Red }
  Write-Host "  A failing case leaves a screenshot, the element tree, the app's log and the exception in clients/windows/Mailcal.UiTests/failures." -ForegroundColor Red
  exit 1
}
# A run that measured nothing is a failure: a -Filter matching only excluded suites, or a -Dataset
# naming only the harness on a host with none, leaves a green step that checked nothing.
if ($passed.Count -eq 0) {
  Write-Host 'FAILED: no case passed, so this run proves nothing.' -ForegroundColor Red
  exit 1
}
exit 0
