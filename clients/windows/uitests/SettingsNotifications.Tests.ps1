#!/usr/bin/env pwsh
# Settings → Notifications (docs/settings.md slot 7): the switch that decides whether new mail
# raises a desktop notification.
#
# Why it is here and not in `Mailcal.Tests`: what a pass SAYS is already pinned there
# (NewMailNoticesTests), and that is the half a plain net10.0 assembly can reach. What it cannot
# reach is the client wiring, the category missing from the source list, a panel built but never
# reached, or a switch that opens on a fresh default instead of on the stored choice. Every
# headless gate stays green through all three, and each one either hides the setting or makes it
# look like it did not take.
#
# The toast itself is out of reach of any suite: AppNotificationManager posts to the shell, outside
# this app's UI Automation tree, so nothing here can see one arrive (docs/background-sync.md known
# gap).
#
# The dataset is `showcase`: the setting is the host's and does not depend on an account, and a
# suite must never open real mail.

# The taxonomy's own neighbours, so the assertion states the CONTRACT (Signatures · Notifications ·
# Privacy) rather than an index that moves whenever a category is added above it.
$NotificationsAbove = 'Signatures'
$NotificationsBelow = 'Privacy'

<#
.SYNOPSIS
The Settings category names, in display order.
#>
function Get-NotificationSuiteCategories {
  param([Parameter(Mandatory)] [object] $Dialog)
  Find-UiaElements -Type 'ListItem' -Root $Dialog | ForEach-Object { $_.Current.Name }
}

<#
.SYNOPSIS
The Notifications panel's switch, by AutomationId.
.DESCRIPTION
By id and not by name: a ToggleSwitch takes its automation Name from its Header, and the group
heading above it reads the same, so a name match can bind to the inert TextBlock and pass for a
switch that was never built (uia.ps1 trap 2).
#>
function Get-NotificationsToggle {
  $dialog = Open-SettingsCategory 'Notifications'
  $toggle = Find-UiaElement -AutomationId 'NotificationsToggle' -Root $dialog
  Assert-True ($null -ne $toggle) (
    'the Notifications category must draw the new-mail switch (docs/settings.md slot 7); the ' +
    'panel opened without it')
  $toggle
}

function Get-ToggleState {
  param([Parameter(Mandatory)] [object] $Element)
  $Element.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Current.ToggleState
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'Settings offers Notifications, between Signatures and Privacy'
      Body = {
        $categories = @(Get-NotificationSuiteCategories -Dialog (Get-SettingsDialog))
        Assert-True ($categories -contains 'Notifications') (
          'this client raises new-mail notifications, so docs/settings.md slot 7 applies to it. ' +
          "Settings holds: $($categories -join ' | ')")
        $index = [array]::IndexOf($categories, 'Notifications')
        Assert-Equal $NotificationsAbove $categories[$index - 1] (
          'the taxonomy is decided once and binds every client: Notifications follows Signatures ' +
          "(docs/settings.md). The order is: $($categories -join ' | ')")
        Assert-Equal $NotificationsBelow $categories[$index + 1] (
          'and precedes Privacy, so "Settings → Notifications" names the same place on every ' +
          "platform. The order is: $($categories -join ' | ')")
      }
    },
    @{
      Name = 'the category draws a switch the user can actually move'
      Body = {
        $toggle = Get-NotificationsToggle
        Assert-True $toggle.Current.IsEnabled (
          'a switch that cannot be pressed is not a setting')
        # Through Get-RenderedBounds, never off BoundingRectangle: a collapsed element stays in
        # the tree with an empty rectangle, which reads as a control that is on screen.
        $bounds = Get-RenderedBounds $toggle 'the new-mail notifications switch'
        Assert-GreaterThan 0 $bounds.Width 'the switch must be on screen, not merely in the tree'
      }
    },
    @{
      Name = 'the choice survives leaving the category and coming back'
      Body = {
        # The panel is rebuilt per category (SettingsDialog.ShowCategory), so a build that seeded
        # the switch from a fresh default rather than from the store would look correct until the
        # user came back to it, and the setting would silently be whatever the default is.
        $original = Get-ToggleState (Get-NotificationsToggle)
        try {
          $flipped = $original -eq [System.Windows.Automation.ToggleState]::Off
          Set-UiaToggle (Get-NotificationsToggle) -On:$flipped
          Open-SettingsCategory $NotificationsBelow | Out-Null
          $reopened = Get-ToggleState (Get-NotificationsToggle)
          $want = if ($flipped) {
            [System.Windows.Automation.ToggleState]::On
          } else {
            [System.Windows.Automation.ToggleState]::Off
          }
          Assert-Equal $want $reopened (
            'the switch reads the stored choice, never a default: a panel rebuilt from a default ' +
            'discards what the user just chose, and says nothing about having done so')
        }
        finally {
          # Leave the developer's own preference as this suite found it.
          Set-UiaToggle (Get-NotificationsToggle) -On:($original -eq [System.Windows.Automation.ToggleState]::On)
        }
      }
    }
  )
}
