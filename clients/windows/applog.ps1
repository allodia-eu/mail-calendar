#!/usr/bin/env pwsh
# Reading the app's own log (`docs/logging.md`) from a script, across a rotation.
#
# Dot-source it; it defines functions and does nothing on its own:
#
#     . "$PSScriptRoot/applog.ps1"           # from clients/windows/
#     $session = Get-AppLogSession -Since $launchedAt
#     if ($session.Contains('refresh_calendar:')) { 'this launch has been to the server' }
#
# WHY IT IS NOT `Get-Content app.log`. `Services/Log.cs` rotates before EVERY write, not before
# each session: once the file passes 1 MB it becomes app.log.1 and the process carries on writing
# into a fresh app.log. So a session that logs past the cap has its own `--- session start` banner
# carried out of the current file WHILE IT IS STILL RUNNING, and a reader that opens app.log alone
# then sees a session with no beginning. That is not a corner: one full UI-suite run writes about
# 2 MB, so it rotates twice, and whichever launch is live at the cut loses its banner.
#
# It produces both kinds of wrong answer, which is why this is shared rather than fixed per caller:
#
#   * the banner is gone, so an anchored reader finds nothing and reports that the app never
#     started, or never reached the state it was waiting for. A healthy app, a red suite, and a
#     message accusing the harness. That is what it did to InvitationPreview.Tests.
#   * the banner is gone, so a reader that falls back to "scan the whole file" matches a line from
#     an EARLIER session and answers immediately. The wait it was asked to perform did not happen,
#     which is the precise failure the anchoring exists to prevent.
#
# THE FALLBACK IS CONDITIONAL, and on "app.log holds no banner at all" rather than on "no banner
# recent enough". The second reads app.log.1 on every poll of a startup that has not landed yet,
# a megabyte at a time, several times a second. The first is the rotation cut's own signature: a
# live session's banner is missing from app.log only when a rotation took it, and the next launch
# writes one back, so the expensive path is taken during the cut session and at no other time.
#
# Two files are enough for a live session. Reaching app.log.2 would need one session to write 1 MB
# after its own banner and then another megabyte, which is more than a whole run writes.

# Mirrors MailboxModel.DataDir + Log.Init. The shared log root, not a per-dev-mode store: app.log
# diagnoses whatever ran last on this machine, whichever MAILCAL_DEV_ACCOUNT it ran under.
$AppLogPath = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\logs\app.log'

# `2026-09-15 15:02:48.828 +02:00 [info] --- session start (0.8.2, Arm64, Microsoft Windows ...) ---`
$AppLogBannerPattern = '(?m)^(?<at>\S+ \S+ \S+) \[info\] --- session start'

<#
.SYNOPSIS
One log file's text, or '' if it is not there.
.DESCRIPTION
Opened ReadWrite-shared because the app holds the file open while this reads it; Get-Content is
refused outright.
#>
function Read-AppLogFile {
  param([Parameter(Mandatory)] [string] $Path)
  if (-not (Test-Path -LiteralPath $Path)) { return '' }
  try {
    $stream = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
  }
  catch {
    # Mid-rotation: File.Move has taken the name away between the test and the open. The caller
    # is polling, so the next pass reads the settled pair.
    return ''
  }
  try { (New-Object IO.StreamReader($stream, [Text.Encoding]::UTF8)).ReadToEnd() }
  finally { $stream.Dispose() }
}

<#
.SYNOPSIS
The log text a live session can be spread across: app.log, with app.log.1 in front of it when a
rotation has carried the running session's banner out of the current file.
#>
function Get-AppLogText {
  $text = Read-AppLogFile $AppLogPath
  if ($text -match $AppLogBannerPattern) { return $text }
  (Read-AppLogFile "$AppLogPath.1") + $text
}

<#
.SYNOPSIS
The log lines belonging to the session that started at or after $Since, or '' if none has yet.
.DESCRIPTION
Anchored on the last `--- session start` banner and its TIMESTAMP, rather than on a byte offset
taken before the launch. Both halves are load-bearing:

  * an absolute offset can outlive the bytes it points at, because the log rotates, and the banner
    and the line being waited for are milliseconds apart, so a rotation between them puts the
    answer in a file the offset no longer addresses;
  * the timestamp is what stops the opposite failure. Without it, an app that never started at all
    leaves the PREVIOUS session's banner as the last one in the file, and its lines answer for a
    launch that never happened.
#>
function Get-AppLogSession {
  param([Parameter(Mandatory)] [datetime] $Since)
  $text = Get-AppLogText
  $banners = [regex]::Matches($text, $AppLogBannerPattern)
  if ($banners.Count -eq 0) { return '' }
  $last = $banners[$banners.Count - 1]
  $at = [datetimeoffset]::MinValue
  if (-not [datetimeoffset]::TryParse($last.Groups['at'].Value, [ref] $at)) { return '' }
  # One second of slack for clock granularity; a previous session's banner is many seconds older,
  # because that app had to come up before this one could replace it.
  if ($at -lt ([datetimeoffset] $Since).AddSeconds(-1)) { return '' }
  $text.Substring($last.Index)
}

<#
.SYNOPSIS
The newest session's lines, whenever it started, or '' if the log holds no banner at all.
.DESCRIPTION
For a caller that has no launch time to gate on, only "whatever is running now". Get-AppLogSession
is the stricter one and is what a caller that launched the app should use.
#>
function Get-AppLogNewestSession {
  $text = Get-AppLogText
  $banners = [regex]::Matches($text, $AppLogBannerPattern)
  if ($banners.Count -eq 0) { return '' }
  $text.Substring($banners[$banners.Count - 1].Index)
}
