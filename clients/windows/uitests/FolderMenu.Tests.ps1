# The folder pane's row menus as the user gets them (docs/folder-pane.md, rules 22 and 23).
#
# WHY THIS SUITE. Mailcal.Tests pins which items a row's flags produce (FolderActionsTests), but
# not that the menu reaches a rendered row at all. The menu is built in code on the NavigationView's
# ContextRequested, because a ContextFlyout declared in the item template never opens (see
# MainWindow.Sidebar.cs), and that route is exactly the kind that breaks while every unit test
# stays green.
#
# WHY THE HARNESS. The showcase engine cannot change folders (its providers carry no folder
# writes), so its pane correctly offers nothing. The harness account is a real JMAP account on
# the local Stalwart server, which can.
#
# Nothing here changes a folder: the menus are opened and read, then shut with Escape. Making,
# renaming and deleting are the core's, proven end to end in tests_folder_ops.rs (mailcal-app).

$CatalogDir = Join-Path $PSScriptRoot '../../../messages'
$Account = 'alice@test.local'

. (Join-Path $PSScriptRoot 'appwindows.ps1')

$Catalog = Get-Content -LiteralPath (Join-Path $CatalogDir 'en.json') -Raw -Encoding utf8 | ConvertFrom-Json

# Right-clicks a pane row and returns the names of the items its menu offers, then shuts the menu.
# A real right-click, as Show-RowContextMenu does for a message row: Shift+F10 reaches whichever
# element holds focus, which is not the row under test.
function Get-PaneRowMenu {
  param([Parameter(Mandatory)] [object] $Row)
  $bounds = $Row.Current.BoundingRectangle
  [void][ReadWin.Input]::SetCursorPos([int] ($bounds.X + 40), [int] ($bounds.Y + $bounds.Height / 2))
  Start-Sleep -Milliseconds 120
  [ReadWin.Input]::mouse_event(0x0008, 0, 0, 0, [UIntPtr]::Zero)   # RIGHTDOWN
  [ReadWin.Input]::mouse_event(0x0010, 0, 0, 0, [UIntPtr]::Zero)   # RIGHTUP
  Start-Sleep -Milliseconds 600
  # Scoped to the flyout, by its language-free class name: a sweep of the desktop for MenuItem
  # collects every window's system menu too (Attendees.Tests.ps1 says how that read).
  $flyout = Find-UiaElements -Type 'Menu' -Root ([System.Windows.Automation.AutomationElement]::RootElement) |
    Where-Object { $_.Current.ClassName -eq 'MenuFlyout' } | Select-Object -First 1
  $names = @()
  if ($flyout) {
    $names = @(Find-UiaElements -Type 'MenuItem' -Root $flyout | ForEach-Object { $_.Current.Name })
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    Start-Sleep -Milliseconds 300
  }
  , $names
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'an account whose folders can change offers New folder beside Remove account'
      Body = {
        $row = Find-UiaElement -Root (Get-MailcalWindow) -Name $Account -Type 'ListItem'
        if (-not $row) { throw "the pane has no row for $Account" }
        $menu = Get-PaneRowMenu -Row $row
        Assert-Equal 2 $menu.Count "the account row offers exactly New folder and Remove account, got: $($menu -join ', ')"
        Assert-Equal $Catalog.folder_action_new $menu[0] 'New folder comes first'
        Assert-Equal $Catalog.action_remove_account $menu[1] 'Remove account is still on the row'
      }
    },
    @{
      Name = 'the Inbox takes a new folder and offers no rename, move or delete'
      Body = {
        # The account's own Inbox, not the unified one under All Accounts: that row belongs to no
        # account and has no menu at all.
        $account = Find-UiaElement -Root (Get-MailcalWindow) -Name $Account -Type 'ListItem'
        $inbox = Find-UiaElement -Root $account -Name $Catalog.folder_inbox -Type 'ListItem'
        if (-not $inbox) { throw 'the account shows no Inbox row; is its tree open?' }
        $menu = Get-PaneRowMenu -Row $inbox
        Assert-Equal 1 $menu.Count "a role folder is named and placed by the app, got: $($menu -join ', ')"
        Assert-Equal $Catalog.folder_action_new $menu[0] 'but it takes a subfolder'
      }
    },
    @{
      Name = 'the unified Inbox raises no menu'
      Body = {
        $group = Find-UiaElement -Root (Get-MailcalWindow) -Name $Catalog.sidebar_all_accounts -Type 'ListItem'
        $unified = Find-UiaElement -Root $group -Name $Catalog.folder_inbox -Type 'ListItem'
        if (-not $unified) { throw 'the All Accounts group shows no Inbox row' }
        $menu = Get-PaneRowMenu -Row $unified
        Assert-Equal 0 $menu.Count 'it is a scope, not a folder on any server'
      }
    }
  )
}
