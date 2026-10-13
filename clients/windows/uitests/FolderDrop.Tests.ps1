# Mail dragged onto a folder in the pane moves there (docs/folder-pane.md, rule 24).
#
# WHY THIS SUITE. Which rows take a drop is FolderActions', pinned in Mailcal.Tests, but whether a
# drag over a rendered row reaches that code at all is WinUI's routing, and it can fail with every
# unit test green: a drag over a NavigationViewItem never reaches the NavigationView's own DragOver,
# so handlers placed there leave every row refusing with the stop cursor.
#
# WHY THE HARNESS. The showcase engine performs no mail writes, so a drop there proves only that
# something was dispatched. The harness moves the message on a real server and the list shows it.
#
# WHY A REAL DRAG. No automation pattern starts a drag, so the gesture is real pointer input
# (winapp.ps1): button down on the message, a move across the window, a rest on the folder so its
# DragOver has answered, and the button up there.
#
# The seed's "Filed under Projects" travels to the Inbox and back, so the suite leaves the mailbox
# as it found it.

$Account = 'alice@test.local'
$Subject = 'Filed under Projects'

# The alice account's row for $Folder, as it is drawn now; each call walks the tree again, because
# a drop re-projects the pane and an element read before it may be stale.
function Get-AccountFolderRow {
  param([Parameter(Mandatory)] [string] $Folder)
  $accountRow = Find-UiaElement -Root (Get-MailcalWindow) -Name $Account -Type 'ListItem'
  if (-not $accountRow) { throw "the pane has no row for $Account" }
  $row = Find-UiaElement -Root $accountRow -Name $Folder -Type 'ListItem'
  if (-not $row) { throw "$Account shows no $Folder row; is its tree open?" }
  $row
}

# Opens $Folder and asks the server for it. A move re-reads the folder it left, not the one it
# reached, and opening a folder shows what the store holds, so the destination is current only
# after a sync, which is what the user presses too.
function Open-AccountFolder {
  param([Parameter(Mandatory)] [string] $Folder)
  Invoke-UiaElement -Element (Get-AccountFolderRow -Folder $Folder) | Out-Null
  [void](Wait-MailRowCount)
  $sync = Find-UiaElement -AutomationId 'SelectionSync' -Type 'Button'
  if (-not $sync) { throw 'the toolbar has no Sync button' }
  Invoke-UiaElement -Element $sync | Out-Null
}

function Find-SubjectRow {
  @(Get-MailRows) | Where-Object { $_.Current.Name -like "*$Subject*" } | Select-Object -First 1
}

# Waits until the open list does, or does not, show the fixture. Returns whether it got there.
function Wait-SubjectRow {
  param([switch] $Absent, [int] $TimeoutSec = 30)
  for ($i = 0; $i -lt $TimeoutSec * 4; $i++) {
    $row = $null
    try { $row = Find-SubjectRow } catch { }
    if ([bool]$row -ne [bool]$Absent) { return $true }
    Start-Sleep -Milliseconds 250
  }
  $false
}

# Presses on $From and lets go on $To's own line.
function Invoke-PointerDrag {
  param([Parameter(Mandatory)] [object] $From, [Parameter(Mandatory)] [object] $To)
  $b = Get-RenderedBounds -Element $To -What 'the folder dropped on'
  # Onto the label, clear of the chevron and the unread count at either end of the row. Onto the
  # row's own line, 20 epx down: an open folder's bounds take in its subfolders' rows, and the
  # middle of those is one of them.
  $toX = [int]($b.X + [Math]::Min(120, $b.Width / 3))
  $toY = [int]($b.Y + [Math]::Min($b.Height / 2, (ConvertTo-UiaPixels 20)))
  # The rest before the release is what lets DragOver answer for this row; without it the drop can
  # land before the row has said it takes one.
  Invoke-WinappDrag -From $From -ToPoint $toX, $toY -DwellMs 800
  Start-Sleep -Milliseconds 300
}

$Suite = @{
  Dataset = 'harness'
  # IMAP, whose account pass re-reads every folder, so Sync brings the destination current.
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-imap' }
  Cases   = @(
    @{
      Name = 'a message dropped on the Inbox leaves Projects and arrives in the Inbox'
      Body = {
        Open-AccountFolder -Folder 'Projects'
        $row = Find-SubjectRow
        if (-not $row) { throw "Projects does not show '$Subject'; was the harness reseeded?" }
        Invoke-PointerDrag -From $row -To (Get-AccountFolderRow -Folder 'Inbox')
        Assert-True (Wait-SubjectRow -Absent) 'the dropped message left the folder it was dragged from'
        Open-AccountFolder -Folder 'Inbox'
        Assert-True (Wait-SubjectRow) 'and the Inbox it was dropped on shows it'
      }
    },
    @{
      Name = 'dropped back on Projects, it returns there'
      Body = {
        Open-AccountFolder -Folder 'Inbox'
        $row = Find-SubjectRow
        if (-not $row) { throw "the Inbox does not show '$Subject'; the case before this one did not land" }
        Invoke-PointerDrag -From $row -To (Get-AccountFolderRow -Folder 'Projects')
        Assert-True (Wait-SubjectRow -Absent) 'the message left the Inbox'
        Open-AccountFolder -Folder 'Projects'
        Assert-True (Wait-SubjectRow) 'and Projects holds it again'
      }
    }
  )
}
