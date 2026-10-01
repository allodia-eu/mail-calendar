# Move to folder… on a message's row menu (docs/folder-pane.md rule 24, docs/list-selection.md
# rule 12).
#
# WHY THIS SUITE. Which folders the picker lists is FolderActions', pinned in Mailcal.Tests, but
# the item's visibility is set by the flyout's Opening handler and the move by its Click, and
# neither is linked into that assembly: an unwired handler leaves the item missing, or present and
# inert, with every unit test green.
#
# WHY THE HARNESS. The showcase engine performs no mail writes, so a move there proves only that
# something was dispatched. The harness moves the message on a real server and the list shows it.
#
# The seed's "Filed under Projects" travels to the Inbox and back, so the suite leaves the mailbox
# as it found it.

$CatalogDir = Join-Path $PSScriptRoot '../../../messages'
$Account = 'alice@test.local'
$Subject = 'Filed under Projects'

. (Join-Path $PSScriptRoot 'appwindows.ps1')

$Catalog = Get-Content -LiteralPath (Join-Path $CatalogDir 'en.json') -Raw -Encoding utf8 | ConvertFrom-Json

# The alice account's row for $Folder, as it is drawn now; each call walks the tree again, because
# a move re-projects the pane and an element read before it may be stale.
function Get-AccountFolderRow {
  param([Parameter(Mandatory)] [string] $Folder)
  $accountRow = Find-UiaElement -Root (Get-MailcalWindow) -Name $Account -Type 'ListItem'
  if (-not $accountRow) { throw "the pane has no row for $Account" }
  $row = Find-UiaElement -Root $accountRow -Name $Folder -Type 'ListItem'
  if (-not $row) { throw "$Account shows no $Folder row; is its tree open?" }
  $row
}

# Opens $Folder and asks the server for it: a move re-reads the folder it left, not the one it
# reached, so the destination is current only after a sync.
function Open-AccountFolder {
  param([Parameter(Mandatory)] [string] $Folder)
  Invoke-UiaElement -Element (Get-AccountFolderRow -Folder $Folder) | Out-Null
  [void](Wait-MailRowCount)
  $sync = Find-UiaElement -AutomationId 'SelectionSync' -Type 'Button'
  if (-not $sync) { throw 'the toolbar has no Sync button' }
  Invoke-UiaElement -Element $sync | Out-Null
}

# Waits until the open list does, or does not, show the fixture. Returns whether it got there.
function Wait-SubjectRow {
  param([switch] $Absent, [int] $TimeoutSec = 30)
  for ($i = 0; $i -lt $TimeoutSec * 4; $i++) {
    $row = $null
    try { $row = @(Get-MailRows) | Where-Object { $_.Current.Name -like "*$Subject*" } | Select-Object -First 1 } catch { }
    if ([bool]$row -ne [bool]$Absent) { return $true }
    Start-Sleep -Milliseconds 250
  }
  $false
}

# Opens the fixture's row menu, presses Move to folder…, and returns the picker's choices by label.
# Leaves the picker open.
function Open-MoveToFolder {
  Show-RowContextMenu -Subject $Subject
  $item = $null
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while (-not $item -and $timer.Elapsed.TotalSeconds -lt 10) {
    $item = Find-UiaElements -Type 'MenuItem' |
      Where-Object { $_.Current.Name -eq $Catalog.action_move_to_folder } | Select-Object -First 1
    if (-not $item) { Start-Sleep -Milliseconds 200 }
  }
  if (-not $item) {
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    throw "the row menu offers no '$($Catalog.action_move_to_folder)'"
  }
  Invoke-UiaElement -Element $item | Out-Null
  $list = Wait-UiaElement -AutomationId 'FolderMoveTargets' -TimeoutSec 10
  if (-not $list) { throw 'Move to folder… opened no picker' }
  $list
}

# Presses the picker's choice labelled $Label.
function Select-MoveTarget {
  param([Parameter(Mandatory)] [object] $List, [Parameter(Mandatory)] [string] $Label)
  $choice = Find-UiaElement -Root $List -Name $Label -Type 'ListItem'
  if (-not $choice) { throw "the picker does not list '$Label'" }
  Invoke-UiaElement -Element $choice | Out-Null
  Wait-UiaGone -AutomationId 'FolderMoveTargets' | Out-Null
}

$Suite = @{
  Dataset = 'harness'
  # IMAP, whose account pass re-reads every folder, so Sync brings the destination current.
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-imap' }
  Cases   = @(
    @{
      Name = 'the picker lists the folders that take mail, not the one on screen'
      Body = {
        Open-AccountFolder -Folder 'Projects'
        if (-not (Wait-SubjectRow)) { throw "Projects does not show '$Subject'; was the harness reseeded?" }
        $list = Open-MoveToFolder
        $labels = @(Find-UiaElements -Root $list -Type 'ListItem' | ForEach-Object { $_.Current.Name })
        $cancel = Find-UiaElement -AutomationId 'CloseButton' -Type 'Button'
        if ($cancel) { Invoke-UiaElement -Element $cancel | Out-Null }
        Wait-UiaGone -AutomationId 'FolderMoveTargets' | Out-Null
        Assert-True ($labels -contains $Catalog.folder_inbox) "the Inbox is offered, got: $($labels -join ', ')"
        Assert-True ($labels -notcontains 'Projects') 'the folder the list is showing is not'
        # Both are in the pane, so leaving them out of the picker is the rule and not the seed.
        [void](Get-AccountFolderRow -Folder $Catalog.folder_junk)
        [void](Get-AccountFolderRow -Folder $Catalog.folder_drafts)
        Assert-True ($labels -notcontains $Catalog.folder_junk) 'Junk takes no move (rule 24)'
        Assert-True ($labels -notcontains $Catalog.folder_drafts) 'nor do Drafts'
      }
    },
    @{
      Name = 'moved to the Inbox, it leaves Projects and arrives there'
      Body = {
        Open-AccountFolder -Folder 'Projects'
        if (-not (Wait-SubjectRow)) { throw "Projects does not show '$Subject'" }
        Select-MoveTarget -List (Open-MoveToFolder) -Label $Catalog.folder_inbox
        Assert-True (Wait-SubjectRow -Absent) 'the message left the folder it was moved from'
        Open-AccountFolder -Folder 'Inbox'
        Assert-True (Wait-SubjectRow) 'and the Inbox shows it'
      }
    },
    @{
      Name = 'moved back to Projects, it returns there'
      Body = {
        Open-AccountFolder -Folder 'Inbox'
        if (-not (Wait-SubjectRow)) { throw "the Inbox does not show '$Subject'; the case before this one did not land" }
        Select-MoveTarget -List (Open-MoveToFolder) -Label 'Projects'
        Assert-True (Wait-SubjectRow -Absent) 'the message left the Inbox'
        Open-AccountFolder -Folder 'Projects'
        Assert-True (Wait-SubjectRow) 'and Projects holds it again'
      }
    }
  )
}
