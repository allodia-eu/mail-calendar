#!/usr/bin/env pwsh
# The running app writes a session-start line that NAMES THE BUILD it came from (docs/logging.md →
# "Session marker, and it names the build"). This suite reads the file the app just wrote.
#
# Why it is here and not in `Mailcal.Tests`: the composition is pinned there (SessionMarkerTests),
# but a pure function proves only that the string would be right IF anything called it. The failure
# this file exists for is the other one, `Log.Init` losing the version argument, or nothing
# reaching `Log.Init` at all, which is the same shape as the bug that gave this suite its name: a
# binding that was correct the whole time with nothing assigning to it. Every headless gate stays
# green through it, and so does a screenshot.
#
# `Log.Init` runs in `Program.Main`, before `Application.Start`, so the sink exists before the
# crash handlers that write into it (docs/logging.md). That also means this suite sees a marker
# whatever the app does afterwards, which is why the case below anchors to the live process rather
# than to the file alone. Any dataset would do; `showcase` is used because a suite must never open
# real mail.
#
# It reads the shared log root deliberately: AppPaths.Root keeps app.log out of the per-dev-mode
# isolated store, "so one file diagnoses whatever ran last on this machine".

$LogPath = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\logs\app.log'

# `2026-08-01 08:14:25.602 +02:00 [info] --- session start (0.2.2, Arm64, Microsoft Windows ...) ---`
# The version group is matched loosely here and asserted on shape below, so a failure can print
# WHAT was logged instead of just "no match".
$MarkerPattern =
  '^(?<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3} [+-]\d{2}:\d{2}) \[info\] --- session start \((?<body>.+)\) ---$'

# `0.2.2` unpackaged, `0.2.2 package 0.2.2.17` inside an MSIX, then the device string. Shaped, not
# pinned to today's numbers: a release must never turn this red.
$VersionPattern = '^\d+\.\d+\.\d+( package \d+\.\d+\.\d+\.\d+)?, .+$'

<#
.SYNOPSIS
The newest `--- session start ---` line in app.log, as @{ When = [DateTimeOffset]; Body = string }.
.DESCRIPTION
Only app.log is read, never the .1-.3 backups: rotation runs BEFORE each write, so the marker of a
live session is always in the current file.
#>
function Get-NewestSessionMarker {
  if (-not (Test-Path -LiteralPath $LogPath)) {
    throw "no log at $LogPath, the app never called Log.Init, or it wrote somewhere else"
  }
  $line = Get-Content -LiteralPath $LogPath |
    Where-Object { $_ -match ' --- session start \(' } |
    Select-Object -Last 1
  if (-not $line) { throw "app.log holds no session-start line at all ($LogPath)" }
  if ($line -notmatch $MarkerPattern) {
    throw "the session-start line does not parse, its shape changed: $line"
  }
  @{
    When = [DateTimeOffset]::Parse($Matches.ts, [Globalization.CultureInfo]::InvariantCulture)
    Body = $Matches.body
    Line = $line
  }
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'the running app stamped a session marker of its own'
      Body = {
        # Anchored to the LIVE process, not to a wall-clock window: a stale marker left by an
        # earlier run would otherwise satisfy every assertion below, and this suite would pass
        # against an app that logs nothing at all.
        $app = Get-Process Mailcal -ErrorAction SilentlyContinue |
          Sort-Object StartTime -Descending | Select-Object -First 1
        Assert-True ($null -ne $app) 'the showcase dataset leaves the app running'

        $marker = Get-NewestSessionMarker
        # One second of slack: the process clock and the log clock are read separately.
        $started = ([DateTimeOffset] $app.StartTime).AddSeconds(-1)
        Assert-True ($marker.When -ge $started) (
          "the newest session marker is from $($marker.When), but the running app started at " +
          "$($app.StartTime), this process wrote no marker, so Log.Init was never reached")
      }
    },
    @{
      Name = 'the session marker names the build the log came from'
      Body = {
        $marker = Get-NewestSessionMarker
        Assert-True ($marker.Body -match $VersionPattern) (
          '/VERSION holds the last RELEASED version, so a log with no version in it cannot be ' +
          'pinned to a build and is not actionable as a support artefact (docs/logging.md). ' +
          "Expected `"<x.y.z>[ package <a.b.c.d>], <device>`", logged: $($marker.Body)")
      }
    }
  )
}
