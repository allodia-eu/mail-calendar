#!/usr/bin/env pwsh
# Tests for applog.ps1, the reader that finds a running session's lines in a log that rotates
# underneath it. Run by build-and-run.ps1 (so the `windows` CI job gates them) and standalone:
#
#   ./applog.tests.ps1
#
# Plain assertions rather than Pester, and exit code as the contract, for the reasons
# screenshot-frame.tests.ps1 gives: 0 all passed, 1 something failed.
#
# WHAT IS ACTUALLY UNDER TEST. Every case below is one arrangement of app.log and app.log.1, and
# the only interesting ones are the arrangements a rotation produces. `Services/Log.cs` rotates
# before EVERY write rather than once per session, so a session that logs past 1 MB has its own
# `--- session start` banner moved into app.log.1 while it is still running: from then until the
# next launch, app.log holds that session's later lines and no banner at all.
#
# That state is unreachable from a fixture built by launching the app, which is why this is a unit
# test over files on disk rather than a case in the UI suite. It is also why it exists: the failure
# it guards is invisible until a run happens to be long enough, and then it is a red suite blaming
# the harness for a healthy app, or, worse, a wait that silently did not happen.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'

$script:Failed = 0
$script:Passed = 0

function Assert-Equal($expected, $actual, [string] $what) {
  if ("$expected" -eq "$actual") {
    $script:Passed++
  }
  else {
    $script:Failed++
    Write-Host "  FAIL $what, expected <$expected>, got <$actual>" -ForegroundColor Red
  }
}

function Assert-True($actual, [string] $what) {
  Assert-Equal $true ([bool] $actual) $what
}

# applog.ps1 resolves the log under $env:LOCALAPPDATA at dot-source time, so point that at a
# throwaway directory BEFORE loading it, and put the real one back at the end. Nothing here reads
# or writes the developer's own log.
$RealLocalAppData = $env:LOCALAPPDATA
$Sandbox = Join-Path ([IO.Path]::GetTempPath()) "mailcal-applog-tests-$PID"
$env:LOCALAPPDATA = $Sandbox
$LogDir = Join-Path $Sandbox 'Allodia\MailCalendar\logs'
. (Join-Path $PSScriptRoot 'applog.ps1')

function Set-Log([string] $name, [string[]] $lines) {
  New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
  Set-Content -NoNewline -LiteralPath (Join-Path $LogDir $name) -Value (($lines -join "`n") + "`n")
}

function Clear-Log {
  Remove-Item -Recurse -Force -LiteralPath $LogDir -ErrorAction SilentlyContinue
}

function Banner([string] $at) {
  "$at +02:00 [info] --- session start (0.8.2, Arm64, Microsoft Windows 10.0.26200) ---"
}

try {
  # 1. Nothing logged at all. Not an error: the app has not started yet and the caller is polling.
  Clear-Log
  Assert-Equal '' (Get-AppLogText) 'no log at all reads as empty'
  Assert-Equal '' (Get-AppLogSession -Since (Get-Date)) 'no log at all yields no session'
  Assert-Equal '' (Get-AppLogNewestSession) 'no log at all yields no newest session'

  # 2. The ordinary case: this launch's banner is in app.log, and its lines follow.
  Clear-Log
  Set-Log 'app.log' @(
    (Banner '2026-09-15 15:00:00.000'), 'earlier: refresh_calendar: not this one'
    (Banner '2026-09-15 15:02:48.828'), 'later: refresh_calendar: this one'
  )
  $session = Get-AppLogSession -Since ([datetime]::Parse('2026-09-15T15:02:47'))
  Assert-True ($session.Contains('this one')) 'the live session carries its own lines'
  Assert-True (-not $session.Contains('not this one')) 'and not the previous session"s'

  # 3. THE ROTATION CUT, which is what this file exists for. The banner has been carried into
  #    app.log.1 by a write that crossed the cap, and app.log holds the rest of the same session
  #    with no banner in it. Reading app.log alone found nothing and reported that the app had
  #    never started; that is what failed InvitationPreview.Tests.
  Clear-Log
  Set-Log 'app.log.1' @(
    (Banner '2026-09-15 15:00:00.000'), 'earlier: refresh_calendar: not this one'
    (Banner '2026-09-15 15:02:48.828'), 'cal: page 2026-09-14 materialized=True'
  )
  Set-Log 'app.log' @('notifications: registered', 'later: refresh_calendar: this one')
  $session = Get-AppLogSession -Since ([datetime]::Parse('2026-09-15T15:02:47'))
  Assert-True ($session.StartsWith((Banner '2026-09-15 15:02:48.828'))) `
    'a banner carried into app.log.1 still anchors its session'
  Assert-True ($session.Contains('this one')) 'and the lines it kept writing into app.log are in it'
  Assert-True (-not $session.Contains('not this one')) `
    'and the session before it is still excluded, which the fallback must not undo'

  # 4. The other half of the anchor, and the one the fallback could have broken: a launch that
  #    never happened must read as nothing, not as the newest session lying around.
  Clear-Log
  Set-Log 'app.log.1' @((Banner '2026-09-15 15:02:48.828'), 'cal: page 2026-09-14')
  Set-Log 'app.log' @('notifications: registered')
  Assert-Equal '' (Get-AppLogSession -Since ([datetime]::Parse('2026-09-15T15:10:00'))) `
    'a launch that never happened is not answered by an older session'

  # 5. One second of slack, because the launch clock and the log clock are read separately.
  Clear-Log
  Set-Log 'app.log' @((Banner '2026-09-15 15:02:48.828'), 'refresh_calendar: this one')
  Assert-True ((Get-AppLogSession -Since ([datetime]::Parse('2026-09-15T15:02:49.500'))).Contains('this one')) `
    'a banner a fraction older than the launch time still counts'

  # 6. Get-AppLogNewestSession has no launch time to gate on, so it answers with whatever is
  #    running, and must find the banner across the cut too.
  Clear-Log
  Set-Log 'app.log.1' @((Banner '2026-09-15 15:02:48.828'), 'rebuild_calendar_cache: 191 occurrence(s)')
  Set-Log 'app.log' @('cal: page 2026-09-14')
  $newest = Get-AppLogNewestSession
  Assert-True ($newest.StartsWith((Banner '2026-09-15 15:02:48.828'))) `
    'the newest session is found across the cut'
  Assert-True ($newest.Contains('rebuild_calendar_cache')) 'with the lines that preceded the cut'

  # 7. A rebuild belonging to an EARLIER session must never answer for this one. The reader this
  #    replaced fell back to the top of the file when it found no banner, which returned an
  #    earlier session's line at once: the wait it was asked to perform did not happen.
  Clear-Log
  Set-Log 'app.log.1' @(
    (Banner '2026-09-15 15:00:00.000'), 'rebuild_calendar_cache: 191 occurrence(s)'
    (Banner '2026-09-15 15:02:48.828')
  )
  Set-Log 'app.log' @('cal: page 2026-09-14, no rebuild yet')
  Assert-True (-not (Get-AppLogNewestSession).Contains('rebuild_calendar_cache')) `
    'a rebuild from the session before the cut does not answer for the one after it'

  # 8. The log is read while the app holds it open for writing, which is what Get-Content cannot
  #    do. Assert it rather than trusting the share flags.
  Clear-Log
  Set-Log 'app.log' @((Banner '2026-09-15 15:02:48.828'), 'refresh_calendar: this one')
  $held = [IO.File]::Open((Join-Path $LogDir 'app.log'), 'Open', 'Write', 'Read')
  try {
    Assert-True ((Get-AppLogText).Contains('this one')) 'the log is readable while the app writes to it'
  }
  finally { $held.Dispose() }
}
finally {
  $env:LOCALAPPDATA = $RealLocalAppData
  Remove-Item -Recurse -Force -LiteralPath $Sandbox -ErrorAction SilentlyContinue
}

if ($script:Failed -gt 0) {
  Write-Host "==> applog: $($script:Failed) failed, $($script:Passed) passed" -ForegroundColor Red
  exit 1
}
Write-Host "==> applog: $($script:Passed) passed" -ForegroundColor Green
