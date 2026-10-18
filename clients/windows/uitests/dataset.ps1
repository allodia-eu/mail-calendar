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
#   Reset-AppSurface       what makes one launch safely serve several suites, and, when it cannot,
#                          says so and lets the caller relaunch instead of guessing.
#
# Nothing here asserts. A failure is a throw that names which stage was reached, because "no window",
# "no message list" and "no rows" have three different causes and one message could only name one.

# clients/windows, where showcase.ps1 and control.ps1 live. Resolved from THIS file rather than
# inherited from whoever dot-sourced it, so the dependency is visible and the file stands alone.
$ClientDir = Join-Path $PSScriptRoot '..'

# ---------------------------------------------------------------------------------------------
# Datasets. Start-Dataset returns only once the app is up and the message list has rows, or, on
# first-run, the account-setup form is up. Each is polled, never slept (uia.ps1 header).
# ---------------------------------------------------------------------------------------------

<#
.SYNOPSIS
Block until the dataset is up: past the first-run question, message list drawn, rows in it.
.DESCRIPTION
ONE tree walk per turn answers all three questions. They used to be three sequential waits, and the
first of them cost its full timeout on every showcase launch, the welcome screen cannot appear
there (a build with no preferences store reports the question settled by construction), so probing
for it separately could only ever run out the clock.

THE FIRST-RUN QUESTION. A store that has never been asked puts the welcome screen up ahead of
everything (`AnalyticsConsent.asked`), and it has no message list, so a suite waiting for one fails
with "the message list never appeared", naming the wrong thing entirely. The first-run dataset
clears its store before every launch, so it lands here every time.

It answers with **Get started**, never "Share usage statistics": a test run must not opt this
machine into analytics, and off is the documented default (docs/analytics.md). By automation id,
not by label, so the check holds in whatever language the app comes up in.

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
      # READY MEANS THE WELCOME QUESTION HAS BEEN ANSWERED, above, and not merely that the setup
      # form is on screen. A core that failed to open also draws the form with no welcome screen
      # before it: `AnalyticsAsked` reads true without a core (MailboxModel.Analytics.cs). "The
      # form is up and the welcome screen is not" cannot tell that apart from "the welcome screen
      # has been dealt with", and five Onboarding cases would then assert against a screen no new
      # user sees, four of them passing by doing so on a build with no Allodia registration.
      #
      # Waiting for the press is deterministic rather than optimistic: the store is cleared before
      # every first-run launch, so the question is always open and the welcome screen always comes.
      if ($Dataset -eq 'first-run') {
        $welcomeUp = $tree | Where-Object {
          $_.Current.AutomationId -eq 'WelcomeGetStarted' -and -not $_.Current.IsOffscreen
        }
        $form = $tree | Where-Object {
          $_.Current.AutomationId -eq 'DetectEmail' -and -not $_.Current.IsOffscreen
        }
        if ($settled -and $form -and -not $welcomeUp) { return }
        $stage = if (-not $settled -and $form) {
          'the setup form with no welcome screen before it, which is what a core that failed to open draws'
        }
        elseif (-not $settled) { 'the launch view, which never gave way to the welcome screen' }
        elseif ($welcomeUp) { 'the welcome screen, which never gave way to the account-setup form' }
        else { 'a window, but the account-setup form never came to the front' }
        Start-Sleep -Milliseconds 200
        continue
      }
      $list = $tree | Where-Object { $_.Current.AutomationId -eq 'RowsList' } | Select-Object -First 1
      if ($list) {
        $stage = 'the message list, but no rows ever arrived in it'
        # Rows arrive after the list does.
        if (@(Find-UiaElements -Type 'ListItem' -Root $list).Count -gt 0) { return }
      }
    }
    Start-Sleep -Milliseconds 200
  }
  throw "dataset '$Dataset' was not ready within ${TimeoutSec}s, got as far as: $stage. If the store was just cleared, the app may be sitting on a screen this runner does not know how to get past, run it by hand once and look."
}

<#
.SYNOPSIS
The set of AutomationIds on screen, and the window's WebView2 hosts, as one comparable string.
.DESCRIPTION
This IS the definition of "clean" used below: whatever the tree holds right after a launch. A suite
that leaves a dialog, a composer or the calendar up changes the set, so comparing against the
launch-time value catches leftovers without anyone having to enumerate what each suite opens.

The hosts are counted beside it because a composer's editor can still be closing after the
composer has left the screen, which no AutomationId shows, and a winapp lookup in the next suite
walks into it and times out (uia.ps1, Get-WebViewHostCount).
#>
function Get-SurfaceFingerprint {
  param([object[]] $Tree)
  if (-not $Tree) { $Tree = @(Get-UiaTree) }
  # ON-SCREEN only, and that is the whole difficulty. A NavigationView keeps the page you navigated
  # away from in the tree, so a fingerprint taken over everything never matches again once a suite
  # has visited the calendar: the runner relaunches for that suite and every suite after it, and
  # hands back most of the saving. IsOffscreen is what separates "still built" from "still shown".
  $ids = ($Tree | Where-Object { -not $_.Current.IsOffscreen -and $_.Current.AutomationId } |
      ForEach-Object { $_.Current.AutomationId } | Sort-Object -Unique) -join ','
  "$ids;webview-hosts=$(Get-WebViewHostCount)"
}

<#
.SYNOPSIS
Wait, bounded, for the WebView2 hosts to come back to $Clean's count; $true when the whole
fingerprint then matches.
.DESCRIPTION
Called once nothing on screen is left to put away: what can still differ is an editor whose browser
window has not closed yet, which nothing here can hurry, so it is waited for, and a relaunch follows
only if it never goes.
#>
function Wait-SurfaceSettled {
  param([Parameter(Mandatory)] [string] $Clean, [string] $Now = (Get-SurfaceFingerprint), [int] $TimeoutSec = 15)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($Now -ne $Clean) {
    if ((Get-OnScreenPart $Now) -ne (Get-OnScreenPart $Clean)) { return $false }
    if ($watch.Elapsed.TotalSeconds -ge $TimeoutSec) {
      # Said, because it is the one leftover nothing on screen shows.
      Write-Host "    after ${TimeoutSec}s the window still has $($Now -replace '^.*=', '') WebView2 host(s), where it came up with $($Clean -replace '^.*=', '')" -ForegroundColor DarkGray
      return $false
    }
    Start-Sleep -Milliseconds 250
    $Now = Get-SurfaceFingerprint
  }
  $true
}

<#
.SYNOPSIS
Says what a suite left that keeps the app from its launch state, for a relaunch nobody has to guess
the reason for.
#>
function Write-SurfaceDifference {
  param([Parameter(Mandatory)] [string] $Clean, [Parameter(Mandatory)] [string] $Now)
  $was = (Get-OnScreenPart $Clean) -split ','
  $is = (Get-OnScreenPart $Now) -split ','
  $gone = @($was | Where-Object { $_ -and $_ -notin $is })
  $added = @($is | Where-Object { $_ -and $_ -notin $was })
  Write-Host "    left on screen: $(if ($added) { $added -join ', ' } else { 'nothing' }); gone: $(if ($gone) { $gone -join ', ' } else { 'nothing' })" -ForegroundColor DarkGray
}

<#
.SYNOPSIS
A fingerprint without its WebView2 host count: what is on screen.
#>
function Get-OnScreenPart {
  param([Parameter(Mandatory)] [string] $Print)
  $Print -replace ';webview-hosts=\d+$', ''
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
    $print = Get-SurfaceFingerprint $tree
    if ($print -eq $Clean) { return $true }
    if ((Get-OnScreenPart $print) -eq (Get-OnScreenPart $Clean)) { return Wait-SurfaceSettled $Clean $print }
    $shown = @($tree | Where-Object { -not $_.Current.IsOffscreen })

    # Every ContentDialog carries CloseButton, account setup carries CancelButton and the composer
    # carries DiscardButton; between them they close everything the suites here open over the list.
    # Discard over a composer something was typed in asks first, and the question is the dialog it
    # raised, so its PrimaryButton (Discard) is pressed rather than its CloseButton (Keep editing).
    # Only with no dialog already up: a press behind one does nothing, and that dialog's own
    # PrimaryButton must not be taken for the question's.
    $acted = $false
    $dialogUp = $shown | Where-Object { $_.Current.AutomationId -eq 'CloseButton' }
    $discard = $shown |
      Where-Object { $_.Current.AutomationId -eq 'DiscardButton' -and $_.Current.ControlType -eq $button } |
      Select-Object -First 1
    if ($discard -and -not $dialogUp) {
      # The suites' own close, which also waits for the editor's browser window to go (uia.ps1).
      try { $null = Close-Composer } catch { return $false }
      $acted = $true
    }
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
    if (-not $acted) {
      Write-SurfaceDifference $Clean $print
      return $false
    }
  }
  if (Wait-SurfaceSettled $Clean) { return $true }
  Write-SurfaceDifference $Clean (Get-SurfaceFingerprint)
  $false
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
    'first-run' {
      # The one screen a person sees ONCE. Its namespace is wiped first, because a first run is
      # defined by there being nothing in it: an account or a settled welcome question left by the
      # last run makes the screen unreachable, and the suite would then assert against whatever it
      # landed on instead. The store directory alone is not the namespace: its accounts live in
      # Credential Manager and survive the directory, so the script clears both. control.ps1
      # launches it; MAILCAL_DEV_ACCOUNT comes from the suite's Env and it honours an inherited
      # one. A running app holds the store open, so it goes first.
      Get-Process Mailcal -ErrorAction SilentlyContinue | Stop-Process -Force
      Get-Process Mailcal -ErrorAction SilentlyContinue | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
      & (Join-Path $ClientDir 'clear-dev-namespace.ps1') -Namespace 'dev-first-run'
      & (Join-Path $ClientDir 'control.ps1') home | Out-Null
    }
    default { throw "unknown dataset '$Dataset' (showcase | first-run); a harness suite belongs in Mailcal.UiTests" }
  }
  Wait-DatasetReady $Dataset
}
