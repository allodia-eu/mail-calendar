#!/usr/bin/env pwsh
# A click on a new-mail notification opens the message it names, with the app running and with the
# app quit (docs/background-sync.md, Enforcement).
#
# THE CLICK IS DELIVERED THE WAY THE SHELL DELIVERS IT. The toast is drawn by the shell, outside
# every accessibility tree, so no suite can press it. What the shell does on a press is a COM call:
# it creates the app's registered notification activator and calls
# `INotificationActivationCallback::Activate` with the toast's `launch` arguments. This suite makes
# the same call. A running app answers it from the class object it registered, which the Windows
# App SDK raises as `NotificationInvoked`; with the app quit, COM starts the exe from the
# activator's `LocalServer32` command line and the click arrives as the launch's own activation.
#
# THE ARGUMENTS ARE THE POSTED TOAST'S. They are read back from the shell's notification history,
# so what is clicked is what the app actually posted, not what an encoder in this file thinks it
# should have posted (`Mailcal.Tests` pins the encoding itself). That also makes this the gate that
# a notification reaches the shell at all.
#
# WHY THE COLD LEG REACHES THE HARNESS. COM starts a local server with the ACTIVATING process's
# environment, so the copy it starts here inherits this runner's MAILCAL_DEV_ACCOUNT and
# MAILCAL_EXTRA_CA and opens the harness store. A click by hand is activated by the shell, whose
# environment is the one it logged in with, which is why that one opens the ordinary store
# (docs/debugging.md).
#
# WHY stalwart-imap. IDLE delivers into the running app, and a notification is raised only for mail
# that arrives while the app is open.

$ActivationCallbackIid = '53E31837-6600-4A81-9395-75CFFE746F94'

# A delivery reaches the running app over IDLE and is then announced; both are well inside this.
$PostedWithinSeconds = 60

# The cold leg starts the app, connects the account and opens the message from the store.
$OpenedWithinSeconds = 90

<#
.SYNOPSIS
Git Bash, by what it says rather than by its name (AGENTS.md: a bare `bash` is WSL's launcher).
#>
function Get-HarnessBash {
  $bash = $env:MAILCAL_BASH
  if (-not $bash) {
    $git = Get-Command git -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $git) { throw 'git is not on PATH, so Git Bash cannot be found; set MAILCAL_BASH' }
    $bash = Join-Path (Split-Path (Split-Path $git.Source)) 'bin\bash.exe'
  }
  $version = & $bash --version 2>$null | Select-Object -First 1
  if ($version -notmatch 'GNU bash, version 5') { throw "'$bash' is not bash 5 (it says: $version); set MAILCAL_BASH" }
  $bash
}

<#
.SYNOPSIS
Appends a message with -Subject to alice's Inbox, through `harness.sh deliver`.
#>
function Send-HarnessMessage {
  param([Parameter(Mandatory)] [string] $Subject)
  $script = (Join-Path $PSScriptRoot '../../../scripts/dev/harness.sh') -replace '\\', '/'
  $out = & (Get-HarnessBash) $script deliver --subject $Subject 2>&1
  if ($LASTEXITCODE -ne 0) { throw "harness.sh deliver failed: $($out -join ' ')" }
}

<#
.SYNOPSIS
The running build's notification registration: its AUMID and its activator's CLSID.
.DESCRIPTION
`AppNotificationManager.Register()` writes both for an unpackaged build: the AUMID key carries
`CustomActivator`, and that CLSID's `LocalServer32` names the exe. Matched on the exe, because every
checkout and architecture that has ever registered leaves a key of its own.
#>
function Get-NotificationRegistration {
  param([Parameter(Mandatory)] [string] $Exe)
  # A host that cannot show notifications registers fine and posts nothing, so there is no toast
  # to click (NotificationLaunch.Tests.ps1).
  $logged = (Get-AppLogNewestSession) -split "`n" |
    Where-Object { $_ -match 'notifications: registered' } | Select-Object -Last 1
  if ($logged -match 'system setting Unsupported') {
    Skip-Case "this host cannot show notifications (the running app logged: $($logged.Trim()))"
  }
  $classes = 'HKCU:\Software\Classes'
  $found = @(Get-ChildItem -LiteralPath "$classes\AppUserModelId" | ForEach-Object {
      $clsid = (Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction SilentlyContinue).CustomActivator
      if (-not $clsid) { return }
      $server = Get-ItemProperty -LiteralPath "$classes\CLSID\$clsid\LocalServer32" -ErrorAction SilentlyContinue
      if ($server -and $server.'(default)' -like "`"$Exe`"*") {
        [pscustomobject]@{ Aumid = $_.PSChildName; Clsid = [guid]$clsid }
      }
    })
  if ($found.Count -ne 1) {
    throw "expected one notification registration for $Exe, found $($found.Count). Register() writes it at every launch, so none means the app did not register (app.log: 'notifications: could not register')."
  }
  $found[0]
}

<#
.SYNOPSIS
The `launch` arguments of the toast the app posted for -Subject, waiting for it to be posted.
.DESCRIPTION
Read through Windows PowerShell, whose WinRT projection reaches `ToastNotificationHistory`;
PowerShell 7 has none.
#>
function Wait-PostedToast {
  param([Parameter(Mandatory)] [string] $Aumid, [Parameter(Mandatory)] [string] $Subject)
  $read = {
    param([string] $Aumid)
    [void][Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]
    foreach ($toast in [Windows.UI.Notifications.ToastNotificationManager]::History.GetHistory($Aumid)) {
      $xml = [xml]$toast.Content.GetXml()
      [pscustomobject]@{ Launch = $xml.toast.launch; Texts = @($xml.toast.visual.binding.text) }
    }
  }
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $PostedWithinSeconds) {
    $toast = powershell.exe -NoProfile -NonInteractive -Command $read -args $Aumid |
      Where-Object { $_.Texts -contains $Subject } | Select-Object -First 1
    if ($toast) { return [string]$toast.Launch }
    Start-Sleep -Seconds 1
  }
  throw "no toast for '$Subject' was posted within ${PostedWithinSeconds}s. Check app.log for 'notifications: posted', the Notifications switch in Settings, and that Windows allows this app's notifications."
}

<#
.SYNOPSIS
Calls the app's notification activator as the shell does on a click.
#>
function Invoke-NotificationActivator {
  param(
    [Parameter(Mandatory)] [object] $Registration,
    [Parameter(Mandatory)] [string] $Arguments
  )
  if (-not ('MailcalNotificationActivator' -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

[ComImport, Guid("$ActivationCallbackIid"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IMailcalNotificationActivationCallback
{
    void Activate(
        [MarshalAs(UnmanagedType.LPWStr)] string appUserModelId,
        [MarshalAs(UnmanagedType.LPWStr)] string invokedArgs,
        IntPtr data,
        uint count);
}

public static class MailcalNotificationActivator
{
    public static void Activate(Guid clsid, string aumid, string arguments)
    {
        var server = (IMailcalNotificationActivationCallback)Activator.CreateInstance(
            Type.GetTypeFromCLSID(clsid, true));
        try { server.Activate(aumid, arguments, IntPtr.Zero, 0); }
        finally { Marshal.ReleaseComObject(server); }
    }
}
"@
  }
  [MailcalNotificationActivator]::Activate($Registration.Clsid, $Registration.Aumid, $Arguments)
}

<#
.SYNOPSIS
Waits for the reading pane's heading to name -Subject, and says whether it did.
#>
function Wait-ReadingSubject {
  param([Parameter(Mandatory)] [string] $Subject, [int] $TimeoutSec = 30)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $heading = $null
    try { $heading = Find-UiaElement -AutomationId 'ReadingSubject' } catch { }
    if ($heading -and -not $heading.Current.IsOffscreen -and $heading.Current.Name -eq $Subject) { return $true }
    Start-Sleep -Milliseconds 250
  }
  $false
}

<#
.SYNOPSIS
Opens alice's Drafts, asks the server for it, and returns the row titled -Subject, or $null.
#>
function Wait-DraftsRow {
  param([Parameter(Mandatory)] [string] $Subject, [int] $TimeoutSec = 30)
  $account = Find-UiaElement -Root (Get-MailcalWindow) -Name 'alice@test.local' -Type 'ListItem'
  if (-not $account) { throw 'the pane has no row for alice@test.local' }
  $drafts = Find-UiaElement -Root $account -Name 'Drafts' -Type 'ListItem'
  if (-not $drafts) { throw 'alice@test.local shows no Drafts row; is its tree open?' }
  Invoke-UiaElement -Element $drafts | Out-Null
  [void](Wait-MailRowCount -AllowEmpty)
  $sync = Find-UiaElement -AutomationId 'SelectionSync' -Type 'Button'
  if ($sync) { Invoke-UiaElement -Element $sync | Out-Null }
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $row = $null
    try {
      $row = @(Get-MailRows) | Where-Object { Get-UiaTree $_ | Where-Object { $_.Current.Name -eq $Subject } } |
        Select-Object -First 1
    }
    catch { }
    if ($row) { return $row }
    Start-Sleep -Milliseconds 250
  }
  $null
}

# The mailbox's exe, from the running app, so this follows whatever the runner launched.
function Get-RunningExe {
  $exe = (Get-Process Mailcal -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).Path
  if (-not $exe) { throw 'the client is not running' }
  $exe
}

$Suite = @{
  Dataset = 'harness'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-imap' }
  Cases   = @(
    @{
      Name = 'a click with the app running opens the message, from the calendar'
      Body = {
        $subject = "Windows notification click $(Get-Random)"
        $registration = Get-NotificationRegistration -Exe (Get-RunningExe)
        Send-HarnessMessage -Subject $subject
        $arguments = Wait-PostedToast -Aumid $registration.Aumid -Subject $subject
        # Over the calendar, because a click takes the app to mail before it opens anything, and
        # there neither the row nor the heading is on screen.
        $nav = Find-UiaElement -AutomationId 'NavCalendar'
        if (-not $nav) { throw 'no Calendar entry in the navigation pane' }
        Invoke-UiaElement $nav -SettleMs 1500
        if (-not (Wait-UiaElement -AutomationId 'CalendarViewMenu' -TimeoutSec 30)) { throw 'the calendar did not come up' }
        Invoke-NotificationActivator -Registration $registration -Arguments $arguments
        Assert-True (Wait-ReadingSubject -Subject $subject) (
          "a click on the notification for '$subject' must bring the mailbox back and open that " +
          'message in the reading pane (docs/background-sync.md)')
      }
    }
    @{
      Name = 'a click over a composer keeps what was written in Drafts and opens the message'
      Body = {
        $subject = "Windows notification click over a draft $(Get-Random)"
        $draft = "Notification suite draft $(Get-Random)"
        $registration = Get-NotificationRegistration -Exe (Get-RunningExe)
        Send-HarnessMessage -Subject $subject
        $arguments = Wait-PostedToast -Aumid $registration.Aumid -Subject $subject
        $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
        if (-not $compose) { throw 'no ComposeButton within 30s' }
        Invoke-UiaElement $compose
        if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) { throw 'no composer within 30s of pressing Compose' }
        Set-UiaText (Find-UiaElement -AutomationId 'SubjectBox') $draft
        try {
          Invoke-NotificationActivator -Registration $registration -Arguments $arguments
          Assert-True (Wait-ReadingSubject -Subject $subject) (
            "a click over a composer must open the message it names, '$subject', in the composer's place")
          Assert-True ($null -eq (Find-UiaElement -AutomationId 'PrimaryButton' -Type 'Button')) (
            'a click over a composer asked a question; leaving a composer keeps the draft unasked ' +
            '(docs/drafts.md)')
          Assert-True ($null -ne (Wait-DraftsRow -Subject $draft)) (
            "the composer a click replaced is not in Drafts as '$draft': leaving must save what was written")
        }
        finally {
          # Taken off the server again through the composer's Discard, so the suite leaves Drafts as
          # it found it.
          Close-Composer | Out-Null
          if ($row = Wait-DraftsRow -Subject $draft -TimeoutSec 5) {
            Invoke-UiaElement $row | Out-Null
            if (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30) { Close-Composer | Out-Null }
          }
        }
      }
    }
    @{
      Name = 'a click that starts the app opens the message'
      Body = {
        $subject = "Windows notification cold click $(Get-Random)"
        $exe = Get-RunningExe
        $registration = Get-NotificationRegistration -Exe $exe
        Send-HarnessMessage -Subject $subject
        $arguments = Wait-PostedToast -Aumid $registration.Aumid -Subject $subject
        # Closed rather than killed: the window's Closed handler unregisters the activator's class
        # object, and only then does COM start a new process for the click.
        Get-Process Mailcal -ErrorAction SilentlyContinue | ForEach-Object { [void]$_.CloseMainWindow() }
        $timer = [Diagnostics.Stopwatch]::StartNew()
        while ((Get-Process Mailcal -ErrorAction SilentlyContinue) -and $timer.Elapsed.TotalSeconds -lt 30) {
          Start-Sleep -Milliseconds 250
        }
        if (Get-Process Mailcal -ErrorAction SilentlyContinue) {
          throw 'the client was still running 30s after being asked to close, so the click would reach it warm'
        }
        Invoke-NotificationActivator -Registration $registration -Arguments $arguments
        $started = Get-CimInstance Win32_Process -Filter "Name='Mailcal.exe'" | Select-Object -First 1
        Assert-True ($started -and $started.CommandLine -match '----AppNotificationActivated:') (
          'the click must start the app through its notification activator; the process running ' +
          "is: $($started.CommandLine)")
        Assert-True (Wait-ReadingSubject -Subject $subject -TimeoutSec $OpenedWithinSeconds) (
          "a click that starts the app must open the message it names, '$subject', once the " +
          'shell is up (Services/NotificationOpenInbox.cs)')
      }
    }
  )
}
