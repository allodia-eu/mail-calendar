#!/usr/bin/env pwsh
# Drive the running WinUI client deterministically, the Windows counterpart of the Android/iOS
# input in scripts/dev/control.sh, but built on the reliable, layout-independent MAILCAL_* launch
# hooks (Mailcal/Services/MailboxModel.Debug.cs) rather than fragile pixel taps (synthetic
# input doesn't drive WinUI reliably). Because the app is single-instanced (Program.cs), a launch
# hook only takes effect in a FRESH process, so each state verb terminates any running instance
# and relaunches the built exe with the hook set, keeping MAILCAL_DEV_ACCOUNT so it stays on the
# harness. `ui-dump` instead reads the live window's UI Automation tree (read-only) for discovery.
#
#   ./control.ps1 open-first        # relaunch + open the first message
#   ./control.ps1 calendar          # relaunch on the calendar
#   ./control.ps1 home              # relaunch on the default all-inboxes view; a fresh connect also
#                                   #   re-syncs the INBOX, so this is how you pick up delivered mail
#   ./control.ps1 swipe archive     # relaunch + swipe the first row (delete | archive | star)
#   ./control.ps1 ui-dump           # print the live window's UI Automation tree (control/name/id)
#
# To ASSERT on the UI (rather than just look at it), dot-source ./uia.ps1, it carries the walk and
# the match rules that keep WinUI's automation tree from handing you a false PASS. Read its header
# before writing a verification script; the traps there are not obvious and three of them are silent.
[CmdletBinding()]
param(
  [Parameter(Mandatory, Position = 0)][string] $Action,
  # The swipe verb's action: delete | archive | star. (Both positions are explicit: once ANY
  # parameter declares a Position, the ones without it stop binding positionally.)
  [Parameter(Position = 1)][string] $Value
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot

# The NEWEST Debug exe, never a Release one. package.ps1 wipes bin/ and rebuilds it Release +
# -p:Packaged=true (the MSIX / framework-dependent-WinAppSDK shape), so after a packaging run the
# newest Mailcal.exe on disk is one that CANNOT run unpackaged: launching it exits immediately and
# every verb here then fails with "no running Mailcal window", as if the app had crashed. The dev
# hooks are `#if DEBUG` anyway, so a Release exe could never honour MAILCAL_* even if it did launch.
function Find-Exe {
  $exe = Get-ChildItem -Path (Join-Path $here 'Mailcal/bin') -Recurse -Filter 'Mailcal.exe' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\Debug\\' } |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $exe) {
    throw "No DEBUG Mailcal.exe under Mailcal/bin, run clients/windows/build-and-run.ps1 first. (A packaging run, package.ps1, cleans bin/ and leaves only Release builds, which can't be driven: the MAILCAL_* hooks are debug-only and the packaged exe won't launch unpackaged.)"
  }
  return $exe.FullName
}

function Stop-Running {
  $running = Get-Process Mailcal -ErrorAction SilentlyContinue
  if ($running) { $running | Stop-Process -Force; Start-Sleep -Milliseconds 500 }
}

# Relaunch the built exe with exactly the given hook set (the app is single-instanced, so the hook
# needs a fresh process). Preserves MAILCAL_DEV_ACCOUNT so the relaunch stays on the harness.
function Invoke-Relaunch([string] $Hook, [string] $HookValue = '1') {
  $exe = Find-Exe
  Stop-Running
  # Clear every hook first, so a verb never inherits the one before it from this shell.
  #
  # MAILCAL_SHOWCASE belongs in this list even though no verb here sets it: showcase.ps1 sets it with
  # `$env:`, which is PROCESS-wide rather than scope-local, so any shell that has taken a screenshot
  # (or launched the showcase dataset) leaves it set for everything that follows. A relaunch that
  # inherits it comes up on the in-memory seed while reporting `account=stalwart`, the harness verb
  # says harness, the window shows fiction, and nothing anywhere disagrees. That is the dataset rule
  # (uia.ps1 header, verify-windows-ui §3) failing silently, and it is worse than a wrong answer,
  # because the showcase engine does not really perform mail actions: a destructive-action test would
  # dispatch into a void and pass. Found when a showcase suite first sorted ahead of a harness one in
  # uitests/ (2026-07-31); the mirror of showcase.ps1 clearing MAILCAL_DEV_ACCOUNT for the same reason.
  #
  # MAILCAL_APPEARANCE is deliberately NOT in the list: it decides only which colours the window is
  # painted in, so carrying it across verbs is what lets a whole suite be driven in dark mode.
  foreach ($v in 'MAILCAL_OPEN_FIRST', 'MAILCAL_CALENDAR', 'MAILCAL_SWIPE',
    'MAILCAL_SHOWCASE', 'MAILCAL_SHOWCASE_SCREEN') {
    Remove-Item "env:$v" -ErrorAction SilentlyContinue
  }
  if ($Hook) { Set-Item "env:$Hook" $HookValue }
  if (-not $env:MAILCAL_DEV_ACCOUNT) { $env:MAILCAL_DEV_ACCOUNT = 'stalwart' }
  # The IMAP harness account is over implicit TLS with a self-signed cert, which the debug core
  # trusts only via MAILCAL_EXTRA_CA. A relaunch inherits that variable from the caller, but this is
  # often run from a shell that never went through boot.sh, so fall back to the PEM harness.sh
  # extracts into the repo, and refuse rather than relaunch into an opaque TLS failure without it.
  if ($env:MAILCAL_DEV_ACCOUNT -eq 'stalwart-imap' -and -not $env:MAILCAL_EXTRA_CA) {
    $ca = Join-Path $here '../../docker/stalwart/tls/harness-ca.pem'
    if (-not (Test-Path -LiteralPath $ca -PathType Leaf)) {
      throw "MAILCAL_DEV_ACCOUNT=stalwart-imap needs the harness IMAP cert, and there is none at $ca, run scripts/dev/harness.sh up."
    }
    $env:MAILCAL_EXTRA_CA = (Resolve-Path -LiteralPath $ca).Path
  }
  Start-Process $exe
  $state = if ($Hook) { "$Hook=$HookValue" } else { '(default view)' }
  Write-Host "==> relaunched: $state, account=$env:MAILCAL_DEV_ACCOUNT" -ForegroundColor Green
}

function Invoke-UiDump {
  # The tree walk (and the traps around it) live in uia.ps1, so the dump and any verification script
  # share ONE implementation. UI Automation lives in the Windows Desktop assemblies (present under
  # Windows PowerShell and a desktop-enabled pwsh), degrade with a clear message rather than a raw
  # type-load error.
  try { . (Join-Path $here 'uia.ps1') }
  catch { throw "UI Automation isn't available in this PowerShell: $($_.Exception.Message)" }
  Show-UiaTree
}

# The swipe gesture cannot be synthesized (SwipeControl needs real touch/pen/precision-touchpad
# input), so this drives it through the MAILCAL_SWIPE launch hook, the same PerformSwipe the
# gesture and the row's context menu call. The undo window is open for ~4s after launch: screenshot
# (or press Undo via uia.ps1) within it to see the deferred state, or wait it out to see the action
# actually dispatched.
function Invoke-Swipe([string] $SwipeAction) {
  $allowed = 'delete', 'archive', 'star'
  if ($SwipeAction -notin $allowed) {
    throw "swipe needs an action: $($allowed -join ' | ')  (e.g. ./control.ps1 swipe archive)"
  }
  Invoke-Relaunch 'MAILCAL_SWIPE' $SwipeAction
}

switch ($Action) {
  'open-first' { Invoke-Relaunch 'MAILCAL_OPEN_FIRST' }
  'calendar'   { Invoke-Relaunch 'MAILCAL_CALENDAR' }
  'home'       { Invoke-Relaunch '' }
  'relaunch'   { Invoke-Relaunch '' }
  'swipe'      { Invoke-Swipe $Value }
  'ui-dump'    { Invoke-UiDump }
  default {
    Write-Error "unknown action '$Action' (open-first|calendar|home|swipe <delete|archive|star>|ui-dump)"
    exit 2
  }
}
