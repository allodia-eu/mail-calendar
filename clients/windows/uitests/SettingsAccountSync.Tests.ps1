#!/usr/bin/env pwsh
# Settings → Accounts: how one mail account is shared with the person's other devices, and what a
# refusal from the core reads like when it is put on screen.
#
# Why it is here and not in `Mailcal.Tests`: that assembly is plain net10.0 and links no WinUI, and
# `MailboxModel` is a WinUI type, so the failure text it assigns, and the order the card is built
# in, are both invisible to it.
#
# The dataset is `harness`, and the refusal is the point of choosing it: a harness run signs in to
# no Allodia account, so asking the core to change an account's sync position is refused every
# time. That makes the error path, normally the hardest thing in a client to reach on purpose,
# deterministic, and it is the path that shipped a generated field name to the user.

. (Join-Path $PSScriptRoot '..\core-features.ps1')
$AllodiaRegistered = @(Get-CoreCargoFeatures -Root (Join-Path $PSScriptRoot '..\..\..')).Count -gt 0

<#
.SYNOPSIS
Every rendered Text under the open Settings dialog, in tree order.
#>
function Get-AccountSyncTexts {
  @(Find-UiaElements -Type 'Text' -Root (Get-SettingsDialog) |
      Where-Object { -not $_.Current.IsOffscreen } | ForEach-Object { $_.Current.Name })
}

$Suite = @{
  Dataset = 'harness'
  Prepare = {
    Open-SettingsCategory -Name 'Accounts' | Out-Null
    Start-Sleep -Milliseconds 800
  }
  Cases   = @(
    @{
      Name = "an account's address comes before the control that shares it"
      Body = {
        if (-not $AllodiaRegistered) { return }
        $address = Get-RenderedBounds (
          Find-UiaElement -Name 'alice@test.local' -Type 'Text' -Root (Get-SettingsDialog)
        ) -What "the account's address"
        $picker = Get-RenderedBounds (
          Find-UiaElement -Type 'Group' -Root (Get-SettingsDialog) |
            Where-Object { -not $_.Current.IsOffscreen } | Select-Object -First 1
        ) -What 'the sync-position control'
        # This panel is a flat stack of cards with no box around either of them, so the heading is
        # the only thing saying which account the rows under it are about. Built the other way
        # round, a second account's control sits under the FIRST account's last row and reads as
        # belonging to it, which renders perfectly and is wrong.
        Assert-True ($address.Top -lt $picker.Top) (
          "the address names the card, so it comes first (address at $($address.Top), " +
          "sync control at $($picker.Top))")
      }
    }
    @{
      Name = 'a refusal from the core reads as a sentence, not as a generated field name'
      Body = {
        if (-not $AllodiaRegistered) { return }
        # Nobody is signed in on a harness run, so this is refused, which is what we are after.
        $paused = Find-UiaElements -Name 'Paused' -Type 'RadioButton' -Root (Get-SettingsDialog) |
          Select-Object -First 1
        Assert-True ($null -ne $paused) 'the sync-position control offers a Paused position'
        Invoke-UiaElement $paused
        Start-Sleep -Milliseconds 1500
        $failure = @(Get-AccountSyncTexts | Where-Object { $_ -match 'Couldn' }) | Select-Object -First 1
        Assert-True ($null -ne $failure) (
          'a refused change says so: silence reads as the change having been made. On screen: ' +
          ((Get-AccountSyncTexts) -join ' | '))
        # UniFFI's C# codegen builds each error's message as "@v1=" + the field, so a client that
        # shows ex.Message puts a generated field name in product copy. CoreError.Describe strips
        # it, and nothing in this repo writes "@v1", it only exists in generated code, so it is
        # invisible in review and appears only when an error path actually runs.
        Assert-True ($failure -notmatch '@v\d+=') (
          'the message must not carry UniFFI''s generated field wrapper, route it through ' +
          "CoreError.Describe. On screen: $failure")
      }
    }
    @{
      Name = 'no failure ever puts an OAuth error code or endpoint jargon in front of a person'
      Body = {
        if (-not $AllodiaRegistered) { return }
        # The screenshot this suite exists for said:
        #   Couldn't check your other devices: oauth endpoint error: invalid_scope
        #   Unable to issue scope mailcal:accounts:read
        # The core now answers with a typed health and the client draws from THAT, so there is no
        # longer a path from an exception's text to a screen. This asserts the absence of the
        # whole class rather than of that one string: a machine-readable OAuth error code, an
        # endpoint's own words, or a scope name is jargon whichever error produced it.
        $shown = Get-AccountSyncTexts
        foreach ($pattern in 'oauth', 'invalid_scope', 'invalid_grant', 'mailcal:', '@v\d+=',
          'endpoint', 'http \d\d\d') {
          $offending = @($shown | Where-Object { $_ -match "(?i)$pattern" })
          Assert-Equal 0 $offending.Count (
            "a person is never shown '$pattern', the core's typed health decides the words, " +
            "and the detail belongs in the diagnostic log. On screen: $($offending -join ' | ')")
        }
      }
    }
  )
}
