#!/usr/bin/env pwsh
# A launch that IS a click on a new-mail notification survives its own startup
# (docs/background-sync.md, docs/client-traps.md).
#
# WHAT THIS CATCHES, AND WHY NOTHING ELSE DOES. Reading the activation of such a launch means
# building an `AppNotificationActivatedEventArgs`, and the Windows App SDK cannot do that unless
# `AppNotificationManager.Register()` has already run in this process. Unregistered, it does not
# throw: it FAILS FAST, `0xc0000409` inside `Microsoft.WindowsAppRuntime.dll`, before `Log.Init`
# has given anything somewhere to write. The app vanishes on the one launch the whole feature
# exists for, and leaves an empty log behind, so it reads as a notification that does nothing
# rather than as a crash. It shipped that way on this branch and was found by hand.
#
# `Mailcal.Tests` cannot see it: the ordering is between two Windows App SDK calls in `Main`, and
# that assembly links neither. `cargo xtask check-notification-registration` pins the ordering in
# the SOURCE, on every host and in every CI job. This is the other half, the one that runs the real
# binary, and it is the only gate anywhere that exercises a notification activation: no suite on
# any platform can click a notification, because the toast is drawn by the shell, outside every
# accessibility tree.
#
# WHY IT ASSERTS ON AN EXIT CODE, AND WAITS SO LONG FOR ONE. The activation marker is real but the
# activation behind it is not, so both builds eventually end: a broken one fails fast after about
# **7 to 8 seconds**, and a fixed one gets far enough to wait for activation data that no shell is
# going to send and gives up after about ten with a `TimeoutException` (`0xE0434352`). The exit
# code is what tells them apart, so the case has to wait for one.
#
# ⚠️ Do not shorten the wait to keep the suite quick. Killing the process before it faults makes
# this case pass on a broken build, which is how it was first written: the failfast was assumed to
# be immediate, it is not, and the suite went green against the very bug it exists for. A run that
# ends in a kill is a case that proved nothing, so it says so rather than passing.
#
# The cost, and it is a real one: a GREEN run still ends in a managed crash and a Windows Error
# Reporting entry, because a fixed build's own way of ending this launch is to throw. There is no
# way around it from here without changing what the product does on an activation it cannot read.
#
# WHY SHOWCASE. It needs no account and opens no mail; it declares the dataset only to join a group
# rather than pay for an app start of its own.

# The command line the shell gives an unpackaged app it is activating from a notification. The
# payload is empty on purpose: what is under test is reading the activation at all, and an argument
# list this app minted would test our own encoder instead (`Mailcal.Tests`, NewMailNoticesTests).
$ActivationArgument = '----AppNotificationActivated:'

# STATUS_STACK_BUFFER_OVERRUN, which is what a `__fastfail` reports. This exact value is the
# regression: an ordinary managed failure is `0xE0434352` and a clean exit is 0, so the assertion
# names the one code that means "the notification platform was not up".
$FailFast = 0xC0000409

# Comfortably past both ways this launch ends: the failfast at about 7 to 8 seconds and the
# timeout at about ten. Measured, not guessed.
$EndsWithinSeconds = 30

<#
.SYNOPSIS
Closes every running client and waits for it to be gone.
.DESCRIPTION
Through CloseMainWindow rather than a kill, because the point is the window's Closed handler:
that is what unregisters the notification platform and so restores the cold-start condition this
case needs. A kill would leave the registration behind and quietly re-create the gate that cannot
fail.
#>
function Stop-MailcalForColdStart {
  param([int] $TimeoutSec = 30)
  Get-Process -Name Mailcal -ErrorAction SilentlyContinue |
    ForEach-Object { [void]$_.CloseMainWindow() }
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    if (-not (Get-MailcalPids)) { return }
    Start-Sleep -Milliseconds 250
  }
  throw "the client was still running ${TimeoutSec}s after being asked to close, so the platform is still registered and this case would pass on a broken build"
}

<#
.SYNOPSIS
The process ids of every Mailcal instance running right now.
#>
function Get-MailcalPids {
  # The leading comma is load-bearing: a function returning an empty array hands the caller $null,
  # because the pipeline unrolls it on the way out. That reaches the parameter below as a null
  # rather than as "nothing was running", and fails the case for a reason that has nothing to do
  # with notifications. Here the empty answer is the NORMAL one, since the case closes the app
  # first.
  , @(Get-Process -Name Mailcal -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
}

<#
.SYNOPSIS
Waits until no Mailcal process outside $Baseline is left.
.DESCRIPTION
A notification activation is a COM activation, so the runtime ANSWERS it by starting another copy
of the app through the registered activator. That copy is not ours to hold a handle on: it is
launched by the COM infrastructure, it finds the app already running, redirects its activation into
it and exits by itself a moment later. Left alone, the runner's between-suite sweep reaches it
while it is still dying and fails with "Access is denied", which ends the whole run red behind a
case that passed. So the suite leaves the process table exactly as it found it.
#>
function Wait-MailcalPidsSettled {
  # AllowEmptyCollection, because the baseline here is normally EMPTY: the case closes the app
  # before it launches anything, so there is nothing running to record. A mandatory [int[]] refuses
  # an empty array, which fails the case for the wrong reason.
  param(
    [Parameter(Mandatory)] [AllowEmptyCollection()] [int[]] $Baseline,
    [int] $TimeoutSec = 30)
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    if (-not (Get-MailcalPids | Where-Object { $_ -notin $Baseline })) { return }
    Start-Sleep -Milliseconds 250
  }
  throw "a process started by the notification activation is still running after ${TimeoutSec}s"
}

<#
.SYNOPSIS
Launches a process as a notification activation and returns the code it ended with.
.DESCRIPTION
Returns $null only when it had not ended within $EndsWithinSeconds, which the case treats as
having proved nothing rather than as a pass. Takes the executable from the running app so this
follows whatever `run-ui-tests.ps1` launched, rather than guessing an architecture or a
configuration out of a path.
#>
function Start-NotificationActivation {
  $exe = (Get-Process Mailcal -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).Path
  if (-not $exe) { throw 'the client is not running, so there is no build to launch a second copy of' }
  # ⚠️ THE APP HAS TO GO FIRST, or this suite is a gate that cannot fail. The failfast happens only
  # when NO process has the notification platform up: a running instance has registered it, and a
  # second process then reads its activation perfectly happily, on the broken build as well as the
  # fixed one. Verified both ways round. Closing it is what restores the condition, because the
  # window's Closed handler unregisters (NewMailNotifier.Disarm). The runner notices the app is
  # gone and relaunches for the next suite, which is what this case costs.
  Stop-MailcalForColdStart
  $baseline = Get-MailcalPids
  $process = Start-Process -FilePath $exe -ArgumentList $ActivationArgument -PassThru
  $exit = $null
  if ($process.WaitForExit($EndsWithinSeconds * 1000)) {
    $exit = $process.ExitCode
  }
  else {
    # Neither ending arrived. Killed so the suite leaves nothing behind, and reported as $null so
    # the case fails rather than reading a kill as a clean bill of health.
    try { $process.Kill() } catch { }
    [void]$process.WaitForExit(10000)
  }
  Wait-MailcalPidsSettled -Baseline $baseline
  return $exit
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'a notification activation does not fail fast on startup'
      Body = {
        $exit = Start-NotificationActivation
        Assert-True ($null -ne $exit) (
          "the launch had not ended after ${EndsWithinSeconds}s, so neither ending was observed " +
          'and this case proved nothing. It must see an exit code to say anything at all.')
        # The concatenation is parenthesised before -f: without that, -f binds to the last string
        # alone and the message reports the placeholder instead of the code.
        Assert-True ($exit -ne $FailFast) ((
            'a launch activated from a notification died with 0x{0:X8}. The notification platform ' +
            'is not registered before the activation is read, so every click that STARTS the app ' +
            'kills it instead, silently: no log, no window (docs/client-traps.md).') -f $exit)
      }
    }
  )
}
