#!/usr/bin/env pwsh
# Capture one store/marketing screenshot of the WinUI client from the seeded in-memory showcase
# dataset, the Windows half of scripts/dev/showcase.sh, which shells out to this once per
# (locale, screen) pair on a Windows host. Relaunches the built exe with the MAILCAL_SHOWCASE
# launch flags (the app is single-instanced, so a flag only takes effect in a FRESH process),
# waits for it to settle, then shoots the window via screenshot.ps1.
#
#   ./showcase.ps1 -Locale en -Screen list -Out shot.png
#   ./showcase.ps1 -Locale nl -Screen reply -Out shot.png -SettleSeconds 12
#
# ---------------------------------------------------------------------------------------------
# Why this script asserts, twice, that showcase mode is really on:
#
# "The app is showing fictional mail" is the whole safety property of a capture run, and NOTHING
# about a screenshot reveals whether it holds. showcase.sh's size floor only rejects a blank
# frame; a screenshot of a real, fully-populated mailbox is a perfectly plausible 300 kB PNG. So
# a stale binary (built before showcase mode existed), a misspelled variable, or an env var that
# never reached the process all fail *silently and photogenically*, the app opens the developer's
# real accounts and the shutter fires. That is not hypothetical: it is exactly what happened when
# this script was written.
#
#   1. BEFORE launching: the built assembly must contain the showcase driver. A stale exe then
#      never opens the real mailbox at all, rather than being caught after the fact.
#   2. AFTER launching: this process must have logged that it entered showcase mode, naming the
#      locale we asked for. Proves the flag reached *this* process, not just that it could have.
#
# Only then does the shutter fire. Both are cheap; neither is optional.
# ---------------------------------------------------------------------------------------------
[CmdletBinding()]
param(
  # Every locale the shared catalog ships, and only those, the same list scripts/dev/showcase.sh
  # keeps in ALL_LOCALES and lib.sh's showcase_marker_for accepts. Listed rather than derived on
  # purpose: a locale is admissible here only once the showcase actually *seeds* it (a
  # ShowcaseLocale variant + showcase_data/<loc>.rs), which is a strictly smaller set than the
  # catalog's, and a capture of unseeded content would come out silently English.
  [Parameter(Mandatory)] [ValidateSet('en', 'nl', 'de', 'fr', 'es', 'it', 'pt')] [string] $Locale,
  # The MAILCAL_SHOWCASE_SCREEN spellings, which are a cross-client contract: the same strings
  # reach the Apple, Android and Windows drivers. Kept in step with scripts/dev/showcase.sh's
  # ALL_SCREENS + EXTRA_SCREENS by scripts/ci/check-showcase-flag.sh.
  [Parameter(Mandatory)] [ValidateSet('list', 'reply', 'settings', 'signatures', 'add-account', 'calendar', 'invitation')] [string] $Screen,
  # Which light/dark appearance the capture is pinned to (MAILCAL_APPEARANCE). The same three
  # spellings every client parses, kept in step with scripts/dev/showcase.sh by
  # scripts/ci/check-showcase-flag.sh.
  #
  # Defaulted to 'light' rather than left unset, because unset does not mean "light" here, it means
  # "whatever this Windows desktop happens to be set to", and a store set shot on a dark desktop is a
  # dark set that every later check passes. The default is what makes a capture reproducible on
  # someone else's machine.
  [ValidateSet('system', 'light', 'dark')] [string] $Appearance = 'light',
  # Where to write the capture. Not required with -NoCapture, which is the only mode that takes none.
  [string] $Out,
  # Launch into showcase mode and run BOTH safety asserts, but don't fire the shutter, for a caller
  # that wants the deterministic dataset rather than a picture of it (uitests/run-ui-tests.ps1).
  #
  # It is a separate switch rather than "pass -Out $null" because the asserts are the point: they are
  # what proves the app is showing FICTIONAL mail, and a caller must not be able to skip them by
  # skipping the capture. Everything above this line still runs.
  #
  # It also unblocks a host the capture cannot serve at all. The pinned 1440x900 logical frame is
  # exactly 2880x1800 at 200% scale, so on a display of that size the window is inflated past the
  # screen by its resize border, centring puts it at x=-13, and screenshot.ps1 correctly refuses a
  # shot whose left column is off-screen black. A capture there needs a smaller frame or a bigger
  # display; a TEST there needs neither.
  [switch] $NoCapture,
  [int] $SettleSeconds = 7
)
if (-not $NoCapture -and -not $Out) {
  throw 'showcase.ps1 needs -Out (where to write the capture), or -NoCapture to launch without one.'
}
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot

# Mirrors MailboxModel.DataDir + Log.Init: %LOCALAPPDATA%\Allodia\MailCalendar\logs\app.log.
$logPath = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\logs\app.log'

function Find-Exe {
  $exe = Get-ChildItem -Path (Join-Path $here 'Mailcal/bin') -Recurse -Filter 'Mailcal.exe' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $exe) { throw "Mailcal.exe isn't built yet, run clients/windows/build-and-run.ps1 first" }
  return $exe.FullName
}

function Stop-Running {
  $running = Get-Process Mailcal -ErrorAction SilentlyContinue
  if ($running) { $running | Stop-Process -Force; Start-Sleep -Milliseconds 500 }
}

# Guard 1: does this build even know what MAILCAL_SHOWCASE_SCREEN is?
#
# .NET string literals sit in the assembly's #US heap as UTF-16LE, but at no guaranteed alignment
# This one lands on an ODD byte offset. So the two obvious searches both report a false negative:
# `Select-String -Encoding unicode` splits binary content on stray newline bytes, and decoding the
# whole file as UTF-16 from offset 0 only ever aligns on even offsets. Search the raw bytes instead:
# Latin-1 round-trips byte 0..255 to char 0..255 exactly, so an ordinal IndexOf over the Latin-1
# rendering of both haystack and needle is an alignment-independent byte search (embedded NULs and
# all), without the O(n*m) hand-rolled loop.
#
# A Debug build is required as well as sufficient: ShowcaseMode.IsOn is `#if DEBUG`-gated, so a
# Release assembly ignores the flag entirely, and, having dropped the driver, won't carry its
# strings either.
function Assert-ShowcaseBinary([string] $exePath) {
  $dll = Join-Path (Split-Path -Parent $exePath) 'Mailcal.dll'
  if (-not (Test-Path $dll)) { throw "no Mailcal.dll next to $exePath" }
  $haystack = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($dll))
  $needle = [Text.Encoding]::Latin1.GetString([Text.Encoding]::Unicode.GetBytes('MAILCAL_SHOWCASE_SCREEN'))
  if ($haystack.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
    throw @"
This Mailcal.dll has no showcase driver in it, it is stale, or a Release build.
Launching it with MAILCAL_SHOWCASE set would open YOUR REAL ACCOUNTS and photograph them.
Rebuild first:  clients/windows/build-and-run.ps1 -NoRun
  assembly: $dll
  built:    $((Get-Item $dll).LastWriteTime)
"@
  }
}

# Guard 2: did *this* process actually enter showcase mode, in the locale we asked for?
#
# The marker comes from the CORE, not this client: boot::build_showcase logs it from inside the
# in-memory engine's constructor and the shared Logger FFI port routes it here, so its presence
# proves the fictional engine was actually built, not merely that a flag was read. Every platform
# checks the same line (scripts/dev/lib.sh SHOWCASE_LOG_MARKER; the three copies are kept in step
# by scripts/ci/check-showcase-flag.sh). Rust's `{locale:?}` renders the variant capitalized.
#
<#
.SYNOPSIS
The log lines belonging to the session that started at or after $since, or '' if there is no such
session yet.
.DESCRIPTION
Anchored on the last `--- session start` banner and its TIMESTAMP, rather than on a byte offset
taken before the launch. Both halves are load-bearing:

  * the log ROTATES at a size cap (docs/logging.md), so an absolute offset can outlive the bytes it
    points at, and the session banner and the line we are looking for are milliseconds apart, so a
    rotation between them puts the answer in a file the offset no longer addresses. That reads as
    "this build never entered showcase mode", which is an alarming and completely false accusation;
  * the timestamp is what stops the opposite failure. Without it, an app that never started at all
    leaves the PREVIOUS session's banner as the last one in the file, and its marker answers for a
    launch that never happened.

Opened ReadWrite-shared because the app holds the file open while we read it.
#>
function Get-SessionLog([datetime] $since) {
  if (-not (Test-Path $logPath)) { return '' }
  $stream = [IO.File]::Open($logPath, 'Open', 'Read', 'ReadWrite')
  try { $text = (New-Object IO.StreamReader($stream, [Text.Encoding]::UTF8)).ReadToEnd() }
  finally { $stream.Dispose() }

  $banners = [regex]::Matches($text, '(?m)^(?<at>\S+ \S+ \S+) \[info\] --- session start')
  if ($banners.Count -eq 0) { return '' }
  $last = $banners[$banners.Count - 1]
  $at = [datetimeoffset]::MinValue
  if (-not [datetimeoffset]::TryParse($last.Groups['at'].Value, [ref] $at)) { return '' }
  # One second of slack for clock granularity; a previous session's banner is many seconds older,
  # because that app had to come up before this one could replace it.
  if ($at -lt ([datetimeoffset] $since).AddSeconds(-1)) { return '' }
  $text.Substring($last.Index)
}

function Assert-ShowcaseRunning([datetime] $since, [int] $TimeoutSec = 30) {
  # Rust's `{locale:?}` renders the ShowcaseLocale variant, so the seeded language appears with its
  # first letter capitalized: En / Nl / De / Fr / Es / It / Pt. Derived from the code rather than
  # listed arm by arm, the twin of lib.sh's showcase_marker_for, so a new catalog locale needs no
  # edit here, only the ValidateSet above. A hardcoded pair was worse than incomplete: it resolved
  # every unknown locale to 'En', so a `-Locale de` run would have compared against the *English*
  # marker.
  $seeded = $Locale.Substring(0, 1).ToUpperInvariant() + $Locale.Substring(1)
  $marker = "showcase (screenshot) app starting (in-memory engine, seeded $seeded sample content)"

  # POLLED, never slept. The marker is written during startup, so this can be answered the moment
  # it lands, and a fixed wait is wrong in both directions: too long is dead time on every launch
  # (the UI suite pays it once per dataset), too short accuses a healthy build of never entering
  # showcase mode.
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($true) {
    $fresh = Get-SessionLog $since
    if ($fresh.IndexOf($marker, [StringComparison]::Ordinal) -ge 0) { return }
    if ($timer.Elapsed.TotalSeconds -ge $TimeoutSec) { break }
    Start-Sleep -Milliseconds 200
  }

  Stop-Running   # never leave a window full of real mail sitting in front of a shutter
  $wrote = if ($fresh) { "$($fresh.Length) bytes in this session's log" } else { 'this launch wrote no session to the log at all, did it start?' }
  throw @"
This launch did NOT enter showcase mode for locale '$Locale' within ${TimeoutSec}s, refusing to take a screenshot.
Whatever is on screen may be real mail. The app has been stopped.
  looked for: $marker
  in:         $logPath ($wrote)
"@
}

$exe = Find-Exe
Assert-ShowcaseBinary $exe

Stop-Running

# Exactly the showcase flags, and nothing else: a stray MAILCAL_DEV_ACCOUNT would point the app at
# the harness and MAILCAL_OPEN_FIRST/_CALENDAR would fight the screen driver for the same surface.
foreach ($v in 'MAILCAL_DEV_ACCOUNT', 'MAILCAL_OPEN_FIRST', 'MAILCAL_CALENDAR') {
  Remove-Item "env:$v" -ErrorAction SilentlyContinue
}
$env:MAILCAL_SHOWCASE = $Locale
$env:MAILCAL_SHOWCASE_SCREEN = $Screen
# Set from the parameter, never inherited: the scrub above exists because an ambient MAILCAL_* is
# the developer's setting leaking into a store asset, and the theme is the one that would do it
# invisibly, the capture would be of the right screen, in the right language, in the wrong scheme.
$env:MAILCAL_APPEARANCE = $Appearance

$launchedAt = Get-Date
Start-Process $exe
Assert-ShowcaseRunning $launchedAt

if ($NoCapture) { return }

# Only the capture path settles. The shutter wants first paint, the list animation and the avatar
# decode finished; an assertion run polls for the thing it is about to measure instead, so making
# it wait here would be pure dead time once per dataset.
Start-Sleep -Seconds $SettleSeconds

& (Join-Path $here 'screenshot.ps1') -Out $Out
