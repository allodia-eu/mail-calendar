#!/usr/bin/env pwsh
# First run: the screen that adds the first mail account (docs/onboarding.md).
#
# Why it is here and not in `Mailcal.Tests`: that assembly is plain net10.0 and links no WinUI, so
# the panel, its order on screen and the automation peer a screen reader reads are all invisible to
# it. The failures this file exists for are exactly those, and the contract names the worst of them
# itself, the whole panel is gated on one question, and gating only PART of it leaves a lone "or
# connect directly" heading under nothing. That renders perfectly. Every headless gate stays green
# through it.
#
# The dataset is `first-run`: an empty namespace, wiped before the launch, because a first run is
# defined by there being nothing in it. Nothing here presses Continue or the sign-in, the first
# would navigate off the screen under test and the second opens a browser at the real account
# service, and neither is needed to state a rule.
#
# KNOWN GAP, and the reason it is one. The Cancel that ends an outstanding sign-in is not asserted
# here: reaching that state means pressing the sign-in line, which opens a browser at the account
# service on every run of this suite. There is no hook that puts the card into its signing-in state
# without one. Verify it by hand, press "Already have one? Sign in", then Cancel, and the card
# comes back with no error. Without it the card sits on "Signing in…" until the core's own cap: a
# custom-scheme redirect has no socket to drop, so closing the browser sends nothing back.

# Whether THIS build carries an Allodia registration, derived the same way the build derived it,
# `Get-CoreCargoFeatures` is what decided whether the cdylib was compiled with the route at all
# (clients/windows/core-features.ps1). Asking it here rather than hardcoding an expectation is what
# lets one suite state the rule for both build shapes, which is the whole point: a build from
# source must show none of the three, and that is a pass, not a gap.
. (Join-Path $PSScriptRoot '..\core-features.ps1')
$AllodiaRegistered = @(Get-CoreCargoFeatures -Root (Join-Path $PSScriptRoot '..\..\..')).Count -gt 0

# The card's own words, from the shipped English catalog. Hardcoded rather than read back from the
# app: a test that asks the app what it says and then checks it says that cannot fail.
$CardTitle = 'Create an Allodia account'
$CardSubtitle = 'Keeps your list of mail accounts the same on your phone and your computer. ' +
'Your mail and your passwords stay on your devices.'
$SignInLine = 'Already have one? Sign in'
$Divider = 'Or connect a mail account directly'

<#
.SYNOPSIS
The topmost rendered element whose Name is $Name, or $null.
.DESCRIPTION
Typed by the caller wherever it matters: matching on Name alone also returns the inert TextBlock
inside a control, which supports no patterns and would pass for a control that was never built
(uia.ps1 trap 2). Ordering below is measured off BoundingRectangle rather than tree order, because
the rule is about what the reader's eye meets first and a tree walk is not evidence of that.
#>
function Get-OnboardingElement {
  param([Parameter(Mandatory)] [string] $Name, [string] $Type)
  Find-UiaElements -Name $Name -Type $Type |
    Where-Object { -not $_.Current.IsOffscreen } | Select-Object -First 1
}

function Get-OnboardingTop {
  param([Parameter(Mandatory)] [string] $Name, [string] $Type, [Parameter(Mandatory)] [string] $What)
  (Get-RenderedBounds (Get-OnboardingElement -Name $Name -Type $Type) -What $What).Top
}

$Suite = @{
  Dataset = 'first-run'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'first-run' }
  Cases   = @(
    @{
      Name = 'the card, the sign-in line and the divider are absent together, or present together'
      Body = {
        $present = @(
          @{ What = 'the recommendation card'; El = (Get-OnboardingElement -Name $CardTitle -Type 'Group') }
          @{ What = 'the sign-in line'; El = (Get-OnboardingElement -Name $SignInLine -Type 'Hyperlink') }
          @{ What = 'the divider'; El = (Get-OnboardingElement -Name $Divider -Type 'Text') }
        )
        $found = @($present | Where-Object { $_.El }).Count
        if ($AllodiaRegistered) {
          Assert-Equal 3 $found (
            'this build carries an Allodia registration, so all three of items 1 to 3 belong on ' +
            'the screen. Missing: ' +
            (($present | Where-Object { -not $_.El } | ForEach-Object { $_.What }) -join ', '))
        }
        else {
          # THE failure this suite exists for. Items 1 to 3 hang off one question
          # (allodia_sign_in_available), and a client that gates only the card leaves a heading
          # naming a choice nobody was offered.
          Assert-Equal 0 $found (
            'this build carries no Allodia registration, so items 1 to 3 of docs/onboarding.md ' +
            'are absent TOGETHER and the screen is the direct route alone. Still drawn: ' +
            (($present | Where-Object { $_.El } | ForEach-Object { $_.What }) -join ', '))
        }
      }
    }
    @{
      Name = 'the screen is in the order the contract fixes: card, sign-in line, divider, address'
      Body = {
        if (-not $AllodiaRegistered) { return }   # nothing above the address field to order
        $card = Get-OnboardingTop -Name $CardTitle -Type 'Group' -What 'the recommendation card'
        $signIn = Get-OnboardingTop -Name $SignInLine -Type 'Hyperlink' -What 'the sign-in line'
        $divider = Get-OnboardingTop -Name $Divider -Type 'Text' -What 'the divider'
        $address = (Get-RenderedBounds (Find-UiaElement -AutomationId 'DetectEmail' -Type 'Edit') `
            -What 'the email-address field').Top
        Assert-True ($card -lt $signIn) (
          "docs/onboarding.md item 2 is a line UNDER the card, not a second card of equal weight " +
          "(card at ${card}, sign-in line at ${signIn})")
        Assert-True ($signIn -lt $divider) (
          "the divider names what FOLLOWS it, so it sits below the way back to an existing " +
          "account (sign-in line at ${signIn}, divider at ${divider})")
        Assert-True ($divider -lt $address) (
          "the direct route is item 4 and may not be promoted above the card (divider at " +
          "${divider}, address field at ${address})")
      }
    }
    @{
      Name = 'the card is one control, labelled with its action and described by its subtitle'
      Body = {
        if (-not $AllodiaRegistered) { return }
        $card = Get-OnboardingElement -Name $CardTitle -Type 'Group'
        Assert-True ($null -ne $card) (
          'the card is one control rather than a heading beside a button, so a screen reader ' +
          'announces the offer and its action together (docs/onboarding.md, Accessibility)')
        # The name is the ACTION. "Recommended" is a marker on the card, and a screen reader that
        # led with it would announce a judgement before saying what the thing is.
        Assert-Equal $CardTitle $card.Current.Name (
          "the card's accessible label carries the action, never the recommendation marker")
        Assert-Equal $CardSubtitle $card.Current.HelpText (
          'the subtitle is the description, so it is announced after the name rather than as a ' +
          'separate unlabelled block')
      }
    }
    @{
      Name = 'the card claims the account list, on phone and computer, and never the web'
      Body = {
        if (-not $AllodiaRegistered) { return }
        # Read back what is ON SCREEN, never the constant above: a rule checked against the string
        # this file already declared is a rule that cannot fail. The card's subtitle is the one
        # place the offer is described, so it is the text these two rules are about.
        $subtitle = (Get-OnboardingElement -Name $CardSubtitle -Type 'Text')
        Assert-True ($null -ne $subtitle) (
          'the card must say what the account DOES, not only offer one (docs/onboarding.md item ' +
          "1). Expected the subtitle: $CardSubtitle")
        $shown = $subtitle.Current.Name
        # Rule 3: the copy may not out-run the capability matrix, and there is no web
        # client. Rule 4: what travels is the account list, never the mail and never a password.
        Assert-True ($shown -notmatch '(?i)\bweb\b') (
          'there is no web client, so no card says "and web" in any locale, the copy may not ' +
          "out-run the capability matrix. The card says: $shown")
        Assert-True ($shown -match '(?i)mail accounts') (
          'the card claims the account LIST; a card claiming storage, backup or "your mail ' +
          'everywhere" describes something the product does not do. The card says: ' + $shown)
      }
    }
    @{
      Name = 'skipping is one action: the direct route is live while the card is up'
      Body = {
        # Not by pressing Continue, that navigates off the screen the rest of this suite is
        # asserting on. The rule it states is the same one: nothing about the card gates the
        # address field, so typing an address and continuing is the WHOLE of declining.
        $box = Find-UiaElement -AutomationId 'DetectEmail' -Type 'Edit'
        Assert-True ($null -ne $box -and $box.Current.IsEnabled) (
          'the email-address field is item 4 of the screen and is never gated behind an answer ' +
          'to the card (docs/onboarding.md, rule 1)')
        Set-UiaText -Element $box -Text 'someone@example.com'
        $continue = Find-UiaElement -AutomationId 'ContinueButton' -Type 'Button'
        Assert-True ($null -ne $continue -and $continue.Current.IsEnabled) (
          'an address alone is enough to continue: there is no confirmation and no second ask')
        Set-UiaText -Element $box -Text ''
      }
    }
  )
}
