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
# Nothing here changes a folder: the menus and the Move to… picker are opened and read, then shut.
# Making, renaming, moving and deleting are the core's, proven end to end in tests_folder_ops.rs
# (mailcal-app).

$CatalogDir = Join-Path $PSScriptRoot '../../../messages'
$Account = 'alice@test.local'

. (Join-Path $PSScriptRoot 'appwindows.ps1')

$Catalog = Get-Content -LiteralPath (Join-Path $CatalogDir 'en.json') -Raw -Encoding utf8 | ConvertFrom-Json

# Right-clicks a pane row and returns its open menu, or $null when it raised none.
# A real right-click, as Show-RowContextMenu does for a message row: Shift+F10 reaches whichever
# element holds focus, which is not the row under test.
function Open-PaneRowMenu {
  param([Parameter(Mandatory)] [object] $Row)
  $bounds = $Row.Current.BoundingRectangle
  # An expanded row's bounds take in its whole subtree, so their middle is one of its folders.
  # The row's own header is the part above its first child row.
  # The child row is not the row's first child in the control view, so it is looked for.
  $header = $bounds.Height
  $child = @(Find-UiaElements -Root $Row -Type 'ListItem') |
    Where-Object { -not $_.Current.BoundingRectangle.IsEmpty -and $_.Current.BoundingRectangle.Y -gt $bounds.Y } |
    Select-Object -First 1
  if ($child) { $header = $child.Current.BoundingRectangle.Y - $bounds.Y }
  [void][ReadWin.Input]::SetCursorPos([int] ($bounds.X + 40), [int] ($bounds.Y + $header / 2))
  Start-Sleep -Milliseconds 120
  [ReadWin.Input]::mouse_event(0x0008, 0, 0, 0, [UIntPtr]::Zero)   # RIGHTDOWN
  [ReadWin.Input]::mouse_event(0x0010, 0, 0, 0, [UIntPtr]::Zero)   # RIGHTUP
  Start-Sleep -Milliseconds 600
  # Scoped to the flyout, by its language-free class name: a sweep of the desktop for MenuItem
  # collects every window's system menu too (Attendees.Tests.ps1 says how that read).
  Find-UiaElements -Type 'Menu' -Root ([System.Windows.Automation.AutomationElement]::RootElement) |
    Where-Object { $_.Current.ClassName -eq 'MenuFlyout' } | Select-Object -First 1
}

# Right-clicks a pane row and returns the names of the items its menu offers, then shuts the menu.
function Get-PaneRowMenu {
  param([Parameter(Mandatory)] [object] $Row)
  $flyout = Open-PaneRowMenu -Row $Row
  $names = @()
  if ($flyout) {
    $names = @(Find-UiaElements -Type 'MenuItem' -Root $flyout | ForEach-Object { $_.Current.Name })
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    Start-Sleep -Milliseconds 300
  }
  , $names
}

# The alice account's row for $Path, opening each folder on the way so the row is on screen.
function Get-AccountFolderRow {
  param([Parameter(Mandatory)] [string[]] $Path)
  $row = Find-UiaElement -Root (Get-MailcalWindow) -Name $Account -Type 'ListItem'
  if (-not $row) { throw "the pane has no row for $Account" }
  foreach ($name in $Path) {
    $tree = $row.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
    if ($tree.Current.ExpandCollapseState.ToString() -ne 'Expanded') {
      $tree.Expand()
      Start-Sleep -Milliseconds 800
    }
    $row = Find-UiaElement -Root $row -Name $name -Type 'ListItem'
    if (-not $row) { throw "$Account shows no $($Path -join ' / ') row" }
  }
  $row
}

# Presses Move to… on $Row and returns the picker's rows as Label -> Enabled, then cancels it.
# Throws unless the picker is the one for $Name, so a click that landed on a neighbouring row
# cannot pass as this folder's answer.
function Get-FolderMoveTargets {
  param([Parameter(Mandatory)] [object] $Row, [Parameter(Mandatory)] [string] $Name)
  $flyout = Open-PaneRowMenu -Row $Row
  if (-not $flyout) { throw 'the row raised no menu' }
  $item = Find-UiaElement -Root $flyout -Name $Catalog.folder_action_move -Type 'MenuItem'
  if (-not $item) {
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    throw "the row menu offers no '$($Catalog.folder_action_move)'"
  }
  Invoke-UiaElement -Element $item | Out-Null
  $list = Wait-UiaElement -AutomationId 'FolderMoveTargets' -TimeoutSec 10
  if (-not $list) { throw 'Move to… opened no picker' }
  $title = $list.Current.Name
  $targets = [ordered]@{}
  foreach ($choice in @(Find-UiaElements -Root $list -Type 'ListItem')) {
    $targets[$choice.Current.Name] = $choice.Current.IsEnabled
  }
  $cancel = Find-UiaElement -AutomationId 'CloseButton' -Type 'Button'
  if ($cancel) { Invoke-UiaElement -Element $cancel | Out-Null }
  Wait-UiaGone -AutomationId 'FolderMoveTargets' | Out-Null
  $expected = $Catalog.folder_move_title.Replace('{name}', $Name)
  if ($title -ne $expected) { throw "the picker opened for the wrong folder: '$title', not '$expected'" }
  $targets
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
    },
    @{
      Name = 'Move to… on a folder inside another leaves out the folder it is in (rule 24)'
      Body = {
        $targets = Get-FolderMoveTargets -Name '2025' -Row (Get-AccountFolderRow -Path 'Archive', '2025')
        $labels = @($targets.Keys)
        Assert-Equal $Catalog.folder_move_top_level $labels[0] "Top level is the root, got: $($labels -join ', ')"
        Assert-True $targets[$Catalog.folder_move_top_level] 'and is offered, since the folder is not there'
        Assert-True ($labels -notcontains 'Archive / 2025') 'the folder itself is no destination'
        # Archive holds 2026, a destination, so it stays as 2026's parent, but not as a choice.
        Assert-True ($labels -contains 'Archive') "Archive is drawn above 2026, got: $($labels -join ', ')"
        Assert-True (-not $targets['Archive']) 'but the folder 2025 already sits in is not offered'
        Assert-True $targets['Archive / 2026'] 'its sibling is'
        Assert-True $targets['Projects'] 'and so is a folder elsewhere'
      }
    },
    @{
      Name = 'Move to… on a top-level folder keeps Top level as the root, not offered (rule 24)'
      Body = {
        $targets = Get-FolderMoveTargets -Name 'Archive' -Row (Get-AccountFolderRow -Path 'Archive')
        $labels = @($targets.Keys)
        Assert-Equal $Catalog.folder_move_top_level $labels[0] "Top level is still the root, got: $($labels -join ', ')"
        Assert-True (-not $targets[$Catalog.folder_move_top_level]) 'but Archive is already there'
        Assert-True ($labels -notcontains 'Archive') 'the folder itself is no destination'
        Assert-True ($labels -notcontains 'Archive / 2026') 'nor is anything inside it'
        Assert-True $targets['Projects'] "another top-level folder is, got: $($labels -join ', ')"
      }
    }
  )
}
