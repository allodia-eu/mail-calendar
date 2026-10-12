#!/usr/bin/env pwsh
# A JMAP account whose pasted password stopped working, given a new one on its page in Settings →
# Accounts (docs/accounts.md rule 11, the expired-sign-in prompt of rule 7): the server refuses the
# stored secret, the window says so, the account's page offers the field, and the new secret
# reconnects the account in place.
#
# Why it is here and not in `Mailcal.Tests`: the field is drawn only for an account the core reports
# as needing to sign in again, which only a server refusing a real request can produce, and what
# matters is that the secret typed reaches the core and that the core's answer clears the prompt.
#
# The dataset is `harness-setup`. The suite sets alice's mailbox up over JMAP through the form,
# changes her password on the server (Set-HarnessPassword), gives the account the new one, and then
# does the same again with the seeded password, so the server and the account both end as they
# began. The server's password is put back in a `finally` whatever happens, because every other
# harness suite signs in with it. The account is removed at the end.

. (Join-Path $PSScriptRoot 'harness-setup.ps1')

$JmapSecretAddress = 'alice@test.local'
$JmapSecretSeeded = 'harness-alice-pw'
$JmapSecretChanged = 'harness-alice-replaced-secret-pw'

<#
.SYNOPSIS
Asks the server for mail, and waits until the window says the account has to sign in again.
#>
function Wait-SignInExpired {
  param([int] $TimeoutSec = 45)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $settings = Find-SettingsDialog
    if ($settings) {
      Invoke-UiaElement (Find-UiaElement -AutomationId 'CloseButton' -Type 'Button' -Root $settings) -SettleMs 800
    }
    $sync = Find-UiaElement -AutomationId 'SelectionSync' -Type 'Button'
    if ($sync) { Invoke-UiaElement $sync -SettleMs 300 }
    $banner = Wait-UiaElement -AutomationId 'SignInExpiredBanner' -TimeoutSec 5
    if ($banner -and -not $banner.Current.IsOffscreen) { return $banner }
  }
  throw "the server refuses the stored secret, and the window never said the account has to sign in again within ${TimeoutSec}s"
}

<#
.SYNOPSIS
Types $Secret into the account page's field and presses its button; returns the dialog root.
#>
function Set-AccountSecret {
  param([Parameter(Mandatory)] [string] $Secret)
  $dialog = Open-SettingsAccount -Address $JmapSecretAddress
  $field = Find-UiaElement -AutomationId 'AccountReplaceSecret' -Root $dialog
  Assert-True ($null -ne $field) (
    "the account's page offers a new secret to a JMAP account whose pasted one stopped working: " +
    'it has no servers to edit and no provider to sign in at')
  Set-UiaText -Element $field -Text $Secret
  $save = Find-UiaElement -Name 'Sign in again' -Type 'Button' -Root (Get-SettingsDialog)
  Invoke-UiaElement $save -SettleMs 3000
  Get-SettingsDialog
}

<#
.SYNOPSIS
Throws unless the account's page no longer asks for a secret and the window's prompt is gone.
#>
function Assert-SignedIn {
  param([Parameter(Mandatory)] [string] $Why)
  $dialog = Open-SettingsAccount -Address $JmapSecretAddress
  Assert-True ($null -eq (Find-UiaElement -AutomationId 'AccountReplaceSecret' -Root $dialog)) (
    "$Why, so the page no longer asks for one: the core reconnected the account and its next " +
    'answer cleared the expired sign-in')
  Invoke-UiaElement (Find-UiaElement -AutomationId 'CloseButton' -Type 'Button' -Root $dialog) -SettleMs 800
  $banner = Find-UiaElement -AutomationId 'SignInExpiredBanner'
  Assert-True ($null -eq $banner -or $banner.Current.IsOffscreen) "$Why, so the window's prompt is gone too"
}

$Suite = @{
  Dataset = 'harness-setup'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'first-run' }
  Cases   = @(
    @{
      Name = 'a JMAP mailbox set up with a password connects'
      Body = {
        Open-ManualSetup -Address $JmapSecretAddress -Type 'JmapChoice'
        Set-UiaText -Element (Find-UiaElement -AutomationId 'JmapEmail' -Type 'Edit') -Text $JmapSecretAddress
        Set-UiaText -Element (Find-UiaElement -AutomationId 'JmapServer' -Type 'Edit') -Text 'http://127.0.0.1:28080'
        # The server is asked whether it offers a sign-in first; the harness's names an issuer no
        # host resolves, so the answer is the password field.
        $password = Wait-UiaElement -AutomationId 'JmapPassword' -TimeoutSec 20
        if (-not $password) { throw 'the JMAP form never offered its password field' }
        Set-UiaText -Element $password -Text $JmapSecretSeeded
        Invoke-UiaElement (Find-UiaElement -AutomationId 'ConnectButton' -Type 'Button')
        Complete-SetupPrompts
        Assert-SignedIn -Why 'the password was right'
      }
    }
    @{
      Name = 'a password changed on the server is asked for on the page, and the new one reconnects'
      Body = {
        try {
          Set-HarnessPassword -User alice -Secret $JmapSecretChanged
          Wait-SignInExpired | Out-Null
          Set-AccountSecret -Secret $JmapSecretChanged | Out-Null
          Assert-SignedIn -Why 'the new secret is the one the server holds'
          # And back, through the same field, so the account ends holding the seeded password.
          Set-HarnessPassword -User alice -Secret $JmapSecretSeeded
          Wait-SignInExpired | Out-Null
          Set-AccountSecret -Secret $JmapSecretSeeded | Out-Null
          Assert-SignedIn -Why 'the seeded secret is the one the server holds again'
        }
        finally { Set-HarnessPassword -User alice -Secret $JmapSecretSeeded }
      }
    }
    @{
      Name = 'a wrong secret is refused and the page keeps asking'
      Body = {
        try {
          Set-HarnessPassword -User alice -Secret $JmapSecretChanged
          Wait-SignInExpired | Out-Null
          $dialog = Set-AccountSecret -Secret 'not-the-password'
          Assert-True ($null -ne (Find-UiaElement -Name "Signing in again didn$([char]0x2019)t work" -Root $dialog)) (
            'a secret the server refuses is said, rather than taken as signed in')
          Assert-True ($null -ne (Find-UiaElement -AutomationId 'AccountReplaceSecret' -Root $dialog)) (
            'and the field stays, since the account still needs one')
        }
        finally { Set-HarnessPassword -User alice -Secret $JmapSecretSeeded }
        Set-AccountSecret -Secret $JmapSecretSeeded | Out-Null
        Assert-SignedIn -Why 'the seeded secret is the one the server holds again'
      }
    }
    @{
      Name = 'removing it returns the window to first-run setup'
      Body = { Remove-OnlyAccount -Address $JmapSecretAddress }
    }
  )
}
