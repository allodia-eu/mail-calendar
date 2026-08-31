#!/usr/bin/env pwsh
# Settings → Allodia account (docs/settings.md slot 1): the category that holds the product's own
# account, and the two states a client draws for it.
#
# Why it is here and not in `Mailcal.Tests`: that assembly is plain net10.0 and links no WinUI, so
# the source list, the panel it dispatches to and the buttons on that panel are all invisible to it.
# The failures this file exists for are exactly those, the category built but never reached, the
# panel reached but drawn without the way out, or the whole category surviving into a build that
# has no route to offer. Every headless gate stays green through all three.
#
# The dataset is `showcase`: the category's content comes from the core and no account is signed in
# there, which is the signed-out state this asserts, and a suite must never open real mail.

# Whether THIS build carries an Allodia registration, derived the same way the build derived it,
# `Get-CoreCargoFeatures` is what decided whether the cdylib was compiled with the route at all
# (clients/windows/core-features.ps1). Asking it here rather than hardcoding an expectation is what
# lets one suite state the rule for both build shapes: a build from source has no category, and
# that is a pass, not a gap.
. (Join-Path $PSScriptRoot '..\core-features.ps1')
$AllodiaRegistered = @(Get-CoreCargoFeatures -Root (Join-Path $PSScriptRoot '..\..\..')).Count -gt 0

<#
.SYNOPSIS
The Settings category names, in display order.
#>
function Get-AllodiaSuiteCategories {
  param([Parameter(Mandatory)] [object] $Dialog)
  @(Find-UiaElements -Type 'ListItem' -Root $Dialog | ForEach-Object { $_.Current.Name })
}

<#
.SYNOPSIS
The names of every Button on the open panel.
.DESCRIPTION
Typed to Button on purpose: matching a name alone would also return the inert TextBlock inside a
control and pass for a button that was never built (uia.ps1 trap 2).
#>
function Get-AllodiaSuiteButtons {
  param([Parameter(Mandatory)] [object] $Dialog)
  @(Find-UiaElements -Type 'Button' -Root $Dialog | ForEach-Object { $_.Current.Name })
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'the Allodia account is the first category, or the build has no route and it is absent'
      Body = {
        $categories = Get-AllodiaSuiteCategories -Dialog (Get-SettingsDialog)
        if ($AllodiaRegistered) {
          Assert-Equal 'Allodia account' $categories[0] (
            'docs/settings.md slot 1 puts the account first, above the mail settings, it is the ' +
            "identity the rest of the app hangs off. The order is: $($categories -join ' | ')")
        }
        else {
          # A build from source. The category goes whole rather than opening an empty panel, which
          # is what a reader would call a broken screen.
          Assert-True (-not ($categories -contains 'Allodia account')) (
            'this build carries no Allodia registration, so there is no route to offer and the ' +
            "category must not be drawn at all. Settings holds: $($categories -join ' | ')")
        }
      }
    },
    @{
      Name = 'signed out, the category offers a way in AND a way to create'
      Body = {
        $categories = Get-AllodiaSuiteCategories -Dialog (Get-SettingsDialog)
        if (-not $AllodiaRegistered) {
          Assert-True (-not ($categories -contains 'Allodia account')) (
            'no registration means no category, so there is no panel to offer anything')
          return
        }
        $buttons = Get-AllodiaSuiteButtons -Dialog (Open-SettingsCategory 'Allodia account')
        # Both, not one. Someone who has never had an account and presses the only button on the
        # panel lands on a form asking for a password they never set, which reads as the app being
        # broken rather than as the wrong button.
        Assert-True ($buttons -contains 'Sign in') (
          "the signed-out panel must offer a way in. It offers: $($buttons -join ' | ')")
        Assert-True ($buttons -contains 'Create an account') (
          'a lone "Sign in" sends someone with no account to a password form they cannot answer. ' +
          "The panel offers: $($buttons -join ' | ')")
      }
    },
    @{
      Name = 'Accounts is mail accounts again, the Allodia card has left it'
      Body = {
        $buttons = Get-AllodiaSuiteButtons -Dialog (Open-SettingsCategory 'Accounts')
        # The account moved out of Accounts and into its own category. Left in both, it would read
        # as two different accounts, and an Allodia account holds no mailbox.
        $strays = $buttons | Where-Object { $_ -in @('Sign in', 'Create an account', 'Manage account', 'Sign out') }
        Assert-True ($strays.Count -eq 0) (
          'the Allodia account has its own category; Accounts holds mail accounts only, and a ' +
          "copy of its controls here would read as a second account. Accounts offers: $($strays -join ' | ')")
      }
    }
  )
}
