#!/usr/bin/env pwsh
# Adding a mail account whose server may offer a sign-in: what the setup form asks for, as the
# server answers it (docs/mail-oauth.md rules 2, 3 and 8).
#
# Why it is here and not only in `Mailcal.Tests`: ImapSignInGate decides what belongs on screen and
# is unit-tested there, but what IS on screen is the view reading it, and a panel left visible, a
# field toggled on the wrong element or a route that skips the refresh all render without an error.
#
# The dataset is `harness`, because every answer here comes from a real server
# (docker/stalwart/README.md, "Signing in to an IMAP account"):
#
#   alice@localhost, detected              sign in, and a password also works
#   alice@test.local at localhost:12993    only pre-registered apps, with the password field
#   a listener that accepts and says nothing   the password field, once the core gives up
#
# The form is opened from the sidebar in a dialog over the running app and closed again by every
# case, and nothing here connects: Connect is never pressed. The sign-in itself is pressed once,
# with the harness's HTTPS front stopped so discovery fails before any browser opens.
#
# KNOWN GAP. Cancel while a sign-in is out is not asserted: reaching that state opens a real browser
# at the sign-in server. Verify it by hand: add alice@localhost, press "Sign in with your provider",
# then Cancel, and the form stays with the button enabled and no warning.

$SetupIds = @(
  'ImapCheckingText', 'ImapSignInButton', 'ImapUsePasswordButton', 'ImapRegistrationNeededNote',
  'ImapSignInFailedBar', 'Password', 'ConnectButton'
)

function Open-ImapSetup {
  $row = Find-UiaElement -AutomationId 'NavAddAccount' -Type 'ListItem'
  if (-not $row) { throw 'the sidebar has no Add account row (#NavAddAccount) to open the form with' }
  Invoke-UiaElement $row -SettleMs 2000
  $box = Wait-UiaElement -AutomationId 'DetectEmail' -Type 'Edit' -TimeoutSec 10
  if (-not $box) { throw 'pressing Add account opened no setup form (#DetectEmail)' }
  $box
}

function Close-ImapSetup {
  foreach ($id in 'DetectCancelButton', 'CancelButton') {
    $cancel = Find-UiaElement -AutomationId $id -Type 'Button'
    if ($cancel -and $cancel.Current.IsEnabled) { Invoke-UiaElement $cancel -SettleMs 1500; break }
  }
  Wait-UiaGone -AutomationId 'AddAccountDialog' -TimeoutSec 5 | Out-Null
}

<#
.SYNOPSIS
Which of the form's sign-in controls are drawn right now, as a set of AutomationIds.
.DESCRIPTION
One walk of the dialog, not of the window: it is polled several times a second, and the mailbox
behind the dialog is most of the window's tree. A collapsed control has no automation peer, so
presence is the test, not IsOffscreen: the form scrolls inside the dialog, and a field below the
fold is drawn and offscreen at once.

The failure bar is the exception. An InfoBar keeps its peer while closed and drops its message, so
it counts as drawn only while its message is in its subtree.
#>
function Get-ImapShown {
  $dialog = Find-UiaElement -AutomationId 'AddAccountDialog'
  if (-not $dialog) { throw 'the add-account dialog is not on screen' }
  $shown = @{}
  $bar = $null
  foreach ($el in Get-UiaTree -Root $dialog) {
    $id = $el.Current.AutomationId
    if ($SetupIds -contains $id) {
      if ($id -eq 'ImapSignInFailedBar') { $bar = $el } else { $shown[$id] = $el }
    }
  }
  if ($bar) {
    $text = [System.Windows.Automation.ControlType]::Text
    $message = Get-UiaTree -Root $bar | Where-Object { $_.Current.ControlType -eq $text -and $_.Current.Name.Length -gt 20 }
    if ($message) { $shown['ImapSignInFailedBar'] = $bar }
  }
  $shown
}

<#
.SYNOPSIS
Polls the form until -Until holds, recording every state it passed through; throws on timeout.
.DESCRIPTION
The record is the point. "No password field before the answer" and "the field never leaves while
the server is asked again" are claims about every frame, not about where the form ends up, so a
case asserts on the whole history.
#>
function Watch-ImapForm {
  param([Parameter(Mandatory)] [scriptblock] $Until, [int] $TimeoutSec = 25, [string] $What)
  $states = [System.Collections.Generic.List[hashtable]]::new()
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $now = Get-ImapShown
    $states.Add($now)
    if (& $Until $now) { return , $states }
    Start-Sleep -Milliseconds 150
  }
  $last = ($states[-1].Keys | Sort-Object) -join ', '
  throw "the form never reached $What within ${TimeoutSec}s; last drawn: [$last]"
}

function Set-ImapField {
  param([Parameter(Mandatory)] [string] $Id, [Parameter(Mandatory)] [AllowEmptyString()] [string] $Text)
  $box = Find-UiaElement -AutomationId $Id -Type 'Edit'
  if (-not $box) { throw "the form has no #$Id to type into" }
  Set-UiaText -Element $box -Text $Text -SettleMs 100
}

function Open-DetectedCard {
  param([Parameter(Mandatory)] [string] $Address)
  $box = Open-ImapSetup
  Set-UiaText -Element $box -Text $Address -SettleMs 200
  Invoke-UiaElement (Find-UiaElement -AutomationId 'ContinueButton' -Type 'Button') -SettleMs 200
  if (-not (Wait-UiaElement -AutomationId 'Username' -Type 'Edit' -TimeoutSec 30)) {
    throw "detection for $Address never reached the account form"
  }
}

function Open-ManualForm {
  param([Parameter(Mandatory)] [string] $Address, [Parameter(Mandatory)] [string] $Port)
  $box = Open-ImapSetup
  Set-UiaText -Element $box -Text $Address -SettleMs 200
  Invoke-UiaElement (Find-UiaElement -AutomationId 'ManualButton' -Type 'Button') -SettleMs 500
  if (-not (Wait-UiaElement -AutomationId 'ImapHost' -Type 'Edit' -TimeoutSec 10)) {
    throw '"Set up manually" opened no manual form'
  }
  Set-ImapField 'ImapPort' $Port
  Set-ImapField 'ImapHost' 'localhost'
}

# A port that completes the TCP handshake and then never says a word, which is what an overloaded
# server or a stalling middlebox looks like to the probe. Nothing accepts: the listen backlog is
# enough for the client's connect to succeed.
function Start-SilentListener {
  $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
  $listener.Start()
  $listener
}

# The security picker's labels are protocol names, the same in every catalog locale.
function Select-ImapSecurity {
  param([Parameter(Mandatory)] [ValidateSet('SSL/TLS', 'STARTTLS')] [string] $Label)
  $picker = Find-UiaElement -AutomationId 'ImapSecurityPicker' -Type 'ComboBox'
  if (-not $picker) { throw 'the manual form has no IMAP security picker' }
  $picker.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
  $item = $null
  for ($i = 0; $i -lt 20 -and -not $item; $i++) {
    Start-Sleep -Milliseconds 150
    $item = Find-UiaElement -Name $Label -Type 'ListItem'
  }
  if (-not $item) { throw "the IMAP security picker offers no '$Label'" }
  $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
  Wait-UiaGone -Name $Label -Type 'ListItem' -TimeoutSec 3 | Out-Null
  Start-Sleep -Milliseconds 300
}

function Invoke-HarnessFront {
  param([Parameter(Mandatory)] [ValidateSet('stop', 'start')] [string] $Verb)
  $out = & docker compose -p mailcal-core-harness $Verb stalwart-oauth-front 2>&1
  if ($LASTEXITCODE -ne 0) { throw "docker compose $Verb stalwart-oauth-front failed: $out" }
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'a server offering both leads with the sign-in and keeps the password behind its own control'
      Body = {
        Open-DetectedCard 'alice@localhost'
        try {
          $states = Watch-ImapForm -What 'the sign-in offer' -Until { param($s) $s.ContainsKey('ImapSignInButton') }
          $early = @($states | Where-Object { $_.ContainsKey('Password') -or $_.ContainsKey('ConnectButton') })
          Assert-Equal 0 $early.Count (
            'no password field and no Connect may be drawn before, or beside, a sign-in the person has not ' +
            'turned down (docs/mail-oauth.md rules 2 and 8)')
          $offer = $states[-1]
          Assert-True ($offer.ContainsKey('ImapUsePasswordButton')) (
            'a server that also takes a password offers "Use a password instead" under the sign-in')

          Invoke-UiaElement $offer['ImapUsePasswordButton'] -SettleMs 300
          $chosen = Get-ImapShown
          Assert-True ($chosen.ContainsKey('Password') -and $chosen.ContainsKey('ConnectButton')) (
            'choosing the password route draws the field and Connect')
          Assert-True ($chosen.ContainsKey('ImapSignInButton')) (
            'the sign-in stays on screen as an ordinary button once a password is chosen')
          Assert-True (-not $chosen.ContainsKey('ImapUsePasswordButton')) (
            'the control that drew the field has nothing left to do')
        }
        finally { Close-ImapSetup }
      }
    }
    @{
      Name = 'the port fields show what the form dials: never the last account''s, and the picker moves them'
      Body = {
        Open-DetectedCard 'alice@localhost'
        try {
          Watch-ImapForm -What 'the sign-in offer' -Until { param($s) $s.ContainsKey('ImapSignInButton') } | Out-Null
          Assert-Equal '12995' (Get-UiaText (Find-UiaElement -AutomationId 'ImapPort' -Type 'Edit')) (
            'the harness autoconfig names IMAP on 12995, which is what this case needs to have on screen first')
          Invoke-UiaElement (Find-UiaElement -AutomationId 'BackButton' -Type 'Button') -SettleMs 300
          Invoke-UiaElement (Find-UiaElement -AutomationId 'ManualButton' -Type 'Button') -SettleMs 500
          Assert-Equal '993' (Get-UiaText (Find-UiaElement -AutomationId 'ImapPort' -Type 'Edit')) (
            'the manual form shows the standard IMAP port, the one it dials, not the last account''s')
          Assert-Equal '465' (Get-UiaText (Find-UiaElement -AutomationId 'SmtpPort' -Type 'Edit')) (
            'the manual form shows the standard submission port, not the last account''s')
          # Writing those ports is the app's own fill, not the person typing one, so the port
          # still follows the picker (docs/account-autodetect.md).
          Select-ImapSecurity 'STARTTLS'
          Assert-Equal '143' (Get-UiaText (Find-UiaElement -AutomationId 'ImapPort' -Type 'Edit')) (
            'on a fresh manual form the IMAP port follows the security picker until somebody types one')
          Select-ImapSecurity 'SSL/TLS'
          Assert-Equal '993' (Get-UiaText (Find-UiaElement -AutomationId 'ImapPort' -Type 'Edit')) (
            'and follows it back')
          Set-ImapField 'ImapPort' '1993'
          Select-ImapSecurity 'STARTTLS'
          Assert-Equal '1993' (Get-UiaText (Find-UiaElement -AutomationId 'ImapPort' -Type 'Edit')) (
            'a port the person typed is theirs, and the picker leaves it alone')
        }
        finally { Close-ImapSetup }
      }
    }
    @{
      Name = 'a server admitting only pre-registered apps says so and draws the password field'
      Body = {
        Open-ManualForm -Address 'alice@test.local' -Port '12993'
        try {
          $states = Watch-ImapForm -What 'the closed-registration line' -Until { param($s) $s.ContainsKey('ImapRegistrationNeededNote') }
          $early = @($states | Where-Object { $_.ContainsKey('Password') -and -not $_.ContainsKey('ImapRegistrationNeededNote') })
          Assert-Equal 0 $early.Count 'the password field waits for the first answer'
          $answer = $states[-1]
          Assert-True ($answer.ContainsKey('Password') -and $answer.ContainsKey('ConnectButton')) (
            'a provider whose sign-in is closed to this app still takes a password here')
          Assert-True (-not $answer.ContainsKey('ImapSignInButton')) 'there is no sign-in this app can start'

          # Rule 8: the field stays while the edited account is asked about again. The line goes at
          # once, because the answer belonged to the old address, which is what proves a second
          # question was asked rather than the first answer kept.
          Set-ImapField 'Username' 'bob@test.local'
          $again = Watch-ImapForm -What 'a second answer' -Until { param($s) $s.ContainsKey('ImapRegistrationNeededNote') }
          $asked = @($again | Where-Object { -not $_.ContainsKey('ImapRegistrationNeededNote') })
          Assert-GreaterThan 0 $asked.Count 'editing the address asks the server again'
          $taken = @($again | Where-Object { -not $_.ContainsKey('Password') -or -not $_.ContainsKey('ConnectButton') })
          Assert-Equal 0 $taken.Count (
            'a password field the form has shown stays, with Connect, while the server is asked again')
        }
        finally { Close-ImapSetup }
      }
    }
    @{
      Name = 'a server that never answers leaves the password field'
      Body = {
        $listener = Start-SilentListener
        try {
          Open-ManualForm -Address 'alice@test.local' -Port "$($listener.LocalEndpoint.Port)"
          try {
            $states = Watch-ImapForm -TimeoutSec 20 -What 'the password field' -Until { param($s) $s.ContainsKey('Password') }
            Assert-True (@($states | Where-Object { $_.ContainsKey('ImapCheckingText') }).Count -gt 0) (
              'while the server is asked the form says so, rather than showing nothing')
            $answer = $states[-1]
            Assert-True ($answer.ContainsKey('ConnectButton')) 'Connect comes with the field'
            Assert-True (-not $answer.ContainsKey('ImapSignInButton') -and -not $answer.ContainsKey('ImapCheckingText')) (
              'an unanswered question is a password form and nothing else (rule 7)')
          }
          finally { Close-ImapSetup }
        }
        finally { $listener.Stop() }
      }
    }
    @{
      Name = 'a failed sign-in says so and hands the password field back'
      Body = {
        Open-DetectedCard 'alice@localhost'
        try {
          $offer = (Watch-ImapForm -What 'the sign-in offer' -Until { param($s) $s.ContainsKey('ImapSignInButton') })[-1]
          # Discovery goes through the HTTPS front, so with it stopped the sign-in fails before a
          # browser is opened.
          Invoke-HarnessFront stop
          try {
            Invoke-UiaElement $offer['ImapSignInButton'] -SettleMs 200
            $failed = (Watch-ImapForm -What 'the failure warning' -Until { param($s) $s.ContainsKey('ImapSignInFailedBar') })[-1]
          }
          finally { Invoke-HarnessFront start }
          Assert-True ($failed.ContainsKey('Password') -and $failed.ContainsKey('ConnectButton')) (
            'a sign-in that did not work brings the password route back, whatever the server said')
          Assert-True ($failed.ContainsKey('ImapSignInButton') -and $failed['ImapSignInButton'].Current.IsEnabled) (
            'the sign-in can be tried again')
        }
        finally { Close-ImapSetup }
      }
    }
  )
}
