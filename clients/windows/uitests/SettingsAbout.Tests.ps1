#!/usr/bin/env pwsh
# Settings → About (docs/settings.md slot 11): the release this build is, where to ask for help,
# and the toolkit this client actually links.
#
# Why it is here and not in `Mailcal.Tests`: `about_info` is already covered in Rust, but a pure
# function proves only that the content would be right IF a client asked for it and drew it. The
# failures this file exists for are the client-side ones, the category missing from the source
# list, the panel built but never reached, or `AboutPlatform.Windows` copied from a sibling client
# and left as another platform's. Every headless gate stays green through all three, and each one
# either hides About from the user or has the app attribute software it does not ship.
#
# The dataset is `showcase`: About's content is the core's and does not depend on an account, and a
# suite must never open real mail.

# /VERSION, the release About must name. `check-version-sync.sh` keeps the crate version equal to
# it, so reading it here states the contract itself, About cannot drift from what a release
# announces, rather than pinning today's number.
$VersionPath = Join-Path $PSScriptRoot '..\..\..\VERSION'

<#
.SYNOPSIS
The Settings category names, in display order.
#>
function Get-SettingsCategories {
  param([Parameter(Mandatory)] [object] $Dialog)
  Find-UiaElements -Type 'ListItem' -Root $Dialog | ForEach-Object { $_.Current.Name }
}

<#
.SYNOPSIS
Opens the About category and returns the dialog root its panel is under.
#>
function Open-AboutCategory {
  Open-SettingsCategory 'About'
}

<#
.SYNOPSIS
Every Text the open panel draws, as one array.
#>
function Get-AboutText {
  param([Parameter(Mandatory)] [object] $Dialog)
  Find-UiaElements -Type 'Text' -Root $Dialog | ForEach-Object { $_.Current.Name }
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'Settings offers About, and offers it last'
      Body = {
        $dialog = Get-SettingsDialog
        $categories = @(Get-SettingsCategories -Dialog $dialog)
        Assert-True ($categories -contains 'About') (
          'docs/settings.md slot 11 puts About in every client. Settings holds: ' +
          ($categories -join ' | '))
        Assert-Equal 'About' $categories[-1] (
          'About is last because it is the thing you go looking for rather than adjust ' +
          "(docs/settings.md). The order is: $($categories -join ' | ')")
      }
    },
    @{
      Name = 'About names the release this build is'
      Body = {
        $version = (Get-Content -LiteralPath $VersionPath -Raw).Trim()
        $dialog = Open-AboutCategory
        $texts = @(Get-AboutText -Dialog $dialog)
        Assert-True ($texts -contains "Version $version") (
          "About must quote the release /VERSION holds, a support answer is built on this " +
          "number, and a client that gets it from anywhere else can differ from what the " +
          "release announced. Expected `"Version $version`", the panel reads: $($texts -join ' / ')")
      }
    },
    @{
      Name = 'About attributes the toolkit this client actually links, and no other'
      Body = {
        $dialog = Open-AboutCategory
        $texts = @(Get-AboutText -Dialog $dialog)
        Assert-True ($texts -contains 'Rust') (
          "every client ships the core, so Rust is attributed everywhere. The panel reads: $($texts -join ' / ')")
        Assert-True ($texts -contains 'Windows App SDK and WinUI') (
          'this client links the Windows App SDK, and an attribution it does not show is a ' +
          "notice it has not given. The panel reads: $($texts -join ' / ')")
        # The failure this catches is passing another client's AboutPlatform, which shows as
        # somebody else's toolkit, not as an empty page, so nothing else would notice.
        $foreign = $texts | Where-Object { $_ -match 'GTK|libadwaita|WebKitGTK|AndroidX|Jetpack' }
        Assert-True ($foreign.Count -eq 0) (
          'Windows links none of these, and attributing software the app does not ship is a ' +
          "false notice. About names: $($foreign -join ' / ')")
      }
    },
    @{
      Name = 'the support address is on the page, with a control that opens it'
      Body = {
        $dialog = Open-AboutCategory
        $texts = @(Get-AboutText -Dialog $dialog)
        Assert-True ($texts -contains 'https://support.allodia.eu') (
          'About exists so someone with a problem can find where to ask. The panel reads: ' +
          ($texts -join ' / '))
        # Typed to Button on purpose: matching the name alone would also return the inert
        # TextBlock inside it and pass for a button that was never built (uia.ps1 trap 2).
        $open = Find-UiaElement -Name 'Open support forum' -Type 'Button' -Root $dialog
        Assert-True ($null -ne $open) (
          'the address is selectable text, but the way out of the app is the button, it is what ' +
          'a phone or a screen reader reaches for')
        Assert-True $open.Current.IsEnabled 'a button that cannot be pressed is not a way out'
      }
    }
  )
}
