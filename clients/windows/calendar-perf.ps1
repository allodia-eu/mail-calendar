#!/usr/bin/env pwsh
# The calendar grid's frame budget, measured, the Windows twin of scripts/dev/calendar-perf.sh.
#
# docs/calendar.md §7: the grid is judged on **the frames it MISSES during motion**, not on its
# average frame and not on a jank ratio. Both of the obvious instruments lied on Android (gfxinfo's
# "Janky frames %" rated two grids equal while one dropped 3x the frames; mpdecimate over a screen
# recording scored the FIXED build worse than the broken one), and Windows has its own version of the
# same trap, see below.
#
# WHY THIS NEEDS A SWAPCHAIN, AND WHY THE GRID HAS ONE
#
#   A WinUI XAML surface, including a Win2D CanvasControl, renders into a DirectComposition
#   surface and owns NO swapchain, so the app presents nothing of its own and DWM composes it.
#   Measured on an ARM64 Surface, elevated: PresentMon captured 451 presents across dwm.exe and
#   WindowsTerminal.exe (which does own a swapchain) and **exactly zero** for ours, through sustained
#   motion in the grid. A grid whose frames cannot be counted cannot be held to a budget.
#
#   So CalendarSurface hosts a CanvasSwapChainPanel and presents into its own swapchain. It now shows
#   up as `Composed: Flip`, with real present timestamps. That is the whole reason for the choice.
#
#   The tempting fallback, timing CompositionTarget.Rendering from inside the app, is the Windows
#   version of the gfxinfo lie: it reports when the **UI thread** ticked, not when a frame was
#   **presented**. Do not use it.
#
# PREREQUISITES
#   - **PresentMon (Intel, required).** This is the only instrument that tells the truth here (see
#     above), so it is a hard prerequisite for Windows calendar-perf work, not an optional extra.
#     Install the Intel PresentMon system package; the CLI then lands at
#     '<ProgramFiles>\Intel\PresentMon\PresentMonApplication\PresentMon.exe', which this script finds
#     automatically. Pass -PresentMon <path> only to override that (e.g. a standalone build from
#     https://github.com/GameTechDev/PresentMon/releases). Note PresentMonUI.exe is the GUI, the
#     script needs the CLI, PresentMon.exe. A missing install fails fast, with the install hint.
#   - **Elevation.** PresentMon opens an ETW session, which needs Administrator or membership of
#     "Performance Log Users". This script raises the UAC prompt for you.
#   - A **Release** build. A Debug build is several times slower, so it tells you about the Debug
#     build. §7 is explicit about this.
#
#   ./calendar-perf.ps1                                  # auto-discovers the system PresentMon
#   ./calendar-perf.ps1 -Seconds 20
#   ./calendar-perf.ps1 -PresentMon C:\tools\PresentMon.exe   # override the discovered path
[CmdletBinding()]
param(
  [string] $PresentMon,
  [int] $Seconds = 14,
  [string] $Out = (Join-Path $env:TEMP "mailcal-frames-$(Get-Random).csv")
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
. (Join-Path $here 'uia.ps1')
. (Join-Path $here 'touch.ps1')

# Resolve the PresentMon CLI up front (before we touch the app), so a missing install fails fast with
# an actionable hint rather than a bare "file not found" three calls deep. An explicit -PresentMon
# wins; otherwise look where the Intel system package installs the CLI (it nests under
# PresentMonApplication\), then fall back to PATH.
function Resolve-PresentMon([string] $Explicit) {
  if ($Explicit) {
    if (-not (Test-Path -LiteralPath $Explicit -PathType Leaf)) {
      throw "-PresentMon '$Explicit' is not a file. Point it at PresentMon.exe (the CLI, not PresentMonUI.exe)."
    }
    return (Resolve-Path -LiteralPath $Explicit).Path
  }
  $roots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}) | Where-Object { $_ }
  foreach ($root in $roots) {
    $candidate = Join-Path $root 'Intel\PresentMon\PresentMonApplication\PresentMon.exe'
    if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
  }
  $onPath = Get-Command 'PresentMon.exe' -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($onPath) { return $onPath.Source }
  throw @"
PresentMon not found. It is required for calendar-perf (the only instrument that reports true
present timing, docs/calendar.md §7). Install the Intel PresentMon system package (its CLI lands at
'$env:ProgramFiles\Intel\PresentMon\PresentMonApplication\PresentMon.exe'), or pass
-PresentMon <path> to a standalone PresentMon.exe from
https://github.com/GameTechDev/PresentMon/releases. (PresentMonUI.exe is the GUI, this needs the CLI.)
"@
}

$PresentMon = Resolve-PresentMon $PresentMon
Write-Host "==> using PresentMon: $PresentMon" -ForegroundColor Cyan

# The grid MUST be on screen before a single contact is injected. This is not paranoia: a horizontal
# swipe on the mail list is a swipe ACTION, and against real accounts it archives real mail. The
# Android script refuses for exactly this reason.
$win = Get-MailcalWindow
if (-not $win) {
  throw 'Mailcal has no window, is the app running? Launch it and open the Calendar, then re-run.'
}
$cond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'CalendarSurface')
if (-not $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)) {
  throw 'The calendar grid is not on screen. Open the Calendar in the app, then re-run, refusing to inject, because a horizontal swipe on the mail list is a swipe ACTION.'
}

$hz = [int]((Get-CimInstance Win32_VideoController).CurrentRefreshRate | Select-Object -First 1)
if (-not $hz) { $hz = 60 }
$budgetMs = 1000.0 / $hz
# A frame is "dropped" when the gap to the next one exceeds 1.5x the budget, the same ratio Android
# uses (12.5ms against an 8.33ms budget at 120Hz).
$dropMs = $budgetMs * 1.5

Write-Host "==> display $hz Hz, budget $([math]::Round($budgetMs,2)) ms, dropped at > $([math]::Round($dropMs,2)) ms" -ForegroundColor Cyan

# Prefer running PresentMon DIRECTLY, no elevation prompt, when this session already holds the
# right to open an ETW session. That is the whole point of joining "Performance Log Users": once you
# have, `pwsh` in your own window can run this with no UAC at all. Elevation is only the fallback for
# a session that lacks the privilege, and a `-Verb RunAs` from a non-interactive host cannot even
# show the consent UI (it returns "cancelled" instantly), so this path is what makes the script
# runnable from tooling.
$id = [Security.Principal.WindowsIdentity]::GetCurrent()
$canTrace =
  ($id.Groups | Where-Object { $_.Value -eq 'S-1-5-32-559' }).Count -gt 0 -or   # Performance Log Users
  (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)

$pmArgs = @('--process_name', 'Mailcal.exe', '--output_file', $Out, '--timed', "$Seconds",
  '--stop_existing_session', '--v1_metrics', '--no_console_stats', '--terminate_after_timed')

if ($canTrace) {
  Write-Host '==> starting PresentMon (this session may open ETW sessions, no prompt)' -ForegroundColor Cyan
  $pm = Start-Process -FilePath $PresentMon -PassThru -WindowStyle Hidden -ArgumentList $pmArgs
} else {
  Write-Host '>>> Approve the UAC prompt (PresentMon needs an ETW session), or join "Performance Log Users" to skip it <<<' -ForegroundColor Yellow
  $pm = Start-Process -FilePath $PresentMon -Verb RunAs -PassThru -ArgumentList $pmArgs
}
Start-Sleep -Seconds 3

Initialize-Touch
$b  = Get-MailcalBounds
$y  = [int]($b.Top  + ($b.Bottom - $b.Top)  * 0.62)
$x1 = [int]($b.Left + ($b.Right  - $b.Left) * 0.78)
$x2 = [int]($b.Left + ($b.Right  - $b.Left) * 0.32)
$cx = [int](($x1 + $x2) / 2)

Write-Host '==> driving the grid (page turns, hour scrolls, a diagonal pinch)' -ForegroundColor Cyan
for ($i = 0; $i -lt 6; $i++) { Invoke-TouchFlick -FromX $x1 -ToX $x2 -Y $y -DurationMs 70; Start-Sleep -Milliseconds 150 }
for ($i = 0; $i -lt 3; $i++) { Invoke-TouchDrag -FromX $cx -FromY ($b.Bottom - 300) -ToX $cx -ToY ($b.Top + 320) -DurationMs 260 }
Invoke-TouchPinch -CenterX $cx -CenterY $y -FromSpread 180 -ToSpread 620 -AngleDeg 45 -DurationMs 700
Invoke-TouchPinch -CenterX $cx -CenterY $y -FromSpread 620 -ToSpread 180 -AngleDeg 45 -DurationMs 700
for ($i = 0; $i -lt 6; $i++) { Invoke-TouchFlick -FromX $x2 -ToX $x1 -Y $y -DurationMs 70; Start-Sleep -Milliseconds 150 }

$pm.WaitForExit(($Seconds + 20) * 1000) | Out-Null
if (-not (Test-Path $Out)) { throw "PresentMon produced no CSV. Is the grid presenting? (A CanvasControl does not.)" }

$rows = Import-Csv $Out
if ($rows.Count -eq 0) { throw 'No presents captured.' }

# **Only the gaps DURING MOTION.** A gap over 60ms means the grid had settled and gone idle, and the
# user simply was not touching it; counting that as a dropped frame is how you end up chasing
# idleness. (Android's author did, and says so.)
$gaps = $rows | ForEach-Object { [double]$_.msBetweenPresents } | Where-Object { $_ -gt 0 -and $_ -lt 60 }
$sorted = $gaps | Sort-Object
$n = $sorted.Count
if ($n -eq 0) { throw 'No in-motion frames captured, was anything actually moving?' }

$dropped = @($gaps | Where-Object { $_ -gt $dropMs }).Count
$p90 = $sorted[[int][math]::Floor($n * 0.90)]
$p99 = $sorted[[int][math]::Floor($n * 0.99)]

Write-Host ''
Write-Host '=== the frames the eye sees ===' -ForegroundColor Green
[pscustomobject]@{
  'presents (total)'          = $rows.Count
  'in-motion frames'          = $n
  "dropped (gap > $([math]::Round($dropMs,1))ms)" = "$dropped  ($([math]::Round(100.0 * $dropped / $n, 1))%)"
  'p90 gap'                   = "$([math]::Round($p90, 1)) ms"
  'p99 gap'                   = "$([math]::Round($p99, 1)) ms"
  'median gap'                = "$([math]::Round($sorted[[int][math]::Floor($n * 0.5)], 1)) ms"
} | Format-List | Out-String | Write-Host

Write-Host "csv: $Out"
