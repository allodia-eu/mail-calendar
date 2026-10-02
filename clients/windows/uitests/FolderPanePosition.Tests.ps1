# A count that moves leaves the folder pane where the reader put it (docs/folder-pane.md).
#
# WHY THIS SUITE. Mailcal.Tests pins that the account list is updated in place (AccountListTests),
# but whether the rendered pane holds its scroll offset is WinUI's: the NavigationView is
# virtualised, a subfolder's row opens a layout pass after its parent is drawn, and a pane that
# takes an account's rows down and puts them back redraws its open subfolders late and shifts
# every row below them. On screen that is the pane moving a moment after a message is dropped on a
# folder, or marked, or arrives.
#
# WHY THE HARNESS. The shift needs open subfolders ABOVE the rows the pane shows, and the showcase
# seed has none. The harness seeds Archive/2025 and Archive/2026 near the top of the pane for this
# (docker/stalwart/README.md, "Nested folders"), and Projects/Clients/Acme holds the message whose
# read state this suite moves.
#
# WHY A SMALL WINDOW. Whether the pane scrolls at all depends on the desktop the suite runs on, so
# the suite sets the window's height, and puts it back afterwards: the app keeps its bounds for the
# next launch.
#
# The message is marked unread and read again, so the suite leaves the mailbox as it found it.

$Account = 'alice@test.local'
$Subject = 'Kickoff notes'
$Opened = @('Archive', 'Projects', 'Clients')

. (Join-Path $PSScriptRoot 'appwindows.ps1')

if (-not ('Allodia.PaneWindow' -as [type])) {
  Add-Type -Namespace Allodia -Name PaneWindow -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool MoveWindow(System.IntPtr h, int x, int y, int w, int ht, bool repaint);
[DllImport("user32.dll")] public static extern bool IsZoomed(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr h, int command);
'@
}

function Get-PaneFolderRow {
  param([Parameter(Mandatory)] [string] $Folder)
  $accountRow = Find-UiaElement -Root (Get-MailcalWindow) -Name $Account -Type 'ListItem'
  if (-not $accountRow) { throw "the pane has no row for $Account" }
  $row = Find-UiaElement -Root $accountRow -Name $Folder -Type 'ListItem'
  if (-not $row) { throw "$Account shows no $Folder row; was the harness seeded with its nested folders?" }
  $row
}

function Set-FolderOpen {
  param([Parameter(Mandatory)] [string] $Folder, [switch] $Shut)
  $pattern = (Get-PaneFolderRow -Folder $Folder).GetCurrentPattern(
    [System.Windows.Automation.ExpandCollapsePattern]::Pattern)
  $open = $pattern.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded
  if ($Shut -and $open) { $pattern.Collapse() }
  if (-not $Shut -and -not $open) { $pattern.Expand() }
  Start-Sleep -Milliseconds 300
}

# The pane's scroller: the scrollable element whose left edge is the window's.
function Get-PaneScroll {
  $window = (Get-MailcalWindow).Current.BoundingRectangle
  $pattern = [System.Windows.Automation.ScrollPattern]::Pattern
  foreach ($el in @(Get-UiaTree)) {
    $r = $el.Current.BoundingRectangle
    if ($r.IsEmpty -or $r.X -gt $window.X + 40 -or $el.GetSupportedPatterns() -notcontains $pattern) { continue }
    $scroll = $el.GetCurrentPattern($pattern)
    if ($scroll.Current.VerticallyScrollable) { return @{ Pattern = $scroll; Bounds = $r } }
  }
  $null
}

# The account's folder rows drawn whole, top to bottom, each as "name@y". A row scrolled past the
# top is not reported empty: its bounds are clipped to what is left of it, a shorter rectangle at
# the top edge, so a row counts when it is as tall as the rows most often are.
function Get-ShownRows {
  param([Parameter(Mandatory)] $Viewport)
  $accountRow = Find-UiaElement -Root (Get-MailcalWindow) -Name $Account -Type 'ListItem'
  $rows = @(Find-UiaElements -Root $accountRow -Type 'ListItem' |
      Where-Object {
        $r = $_.Current.BoundingRectangle
        $_.Current.Name -ne $Account -and -not $r.IsEmpty -and $r.Height -gt 0 `
          -and $r.Top -ge $Viewport.Top - 1 -and $r.Bottom -le $Viewport.Bottom + 1
      })
  if ($rows.Count -eq 0) { return @() }
  $whole = ($rows | Group-Object { [int] $_.Current.BoundingRectangle.Height } |
      Sort-Object Count -Descending | Select-Object -First 1).Name
  $rows |
    Where-Object { $_.Current.BoundingRectangle.Height -ge [int] $whole - 1 } |
    Sort-Object { $_.Current.BoundingRectangle.Y } |
    ForEach-Object { '{0}@{1}' -f $_.Current.Name, [int] $_.Current.BoundingRectangle.Y }
}

# What Acme's row says about its unread mail, '' when it draws no count, and $null while the pane
# has no Acme row at all, which a pane rebuilding its rows has for a moment.
function Get-AcmeUnread {
  try {
    @(Get-UiaTree -Root (Get-PaneFolderRow -Folder 'Acme') |
        Where-Object { $_.Current.Name -match 'unread$' } | ForEach-Object { $_.Current.Name }) -join ''
  }
  catch { $null }
}

# Presses the toolbar's read toggle and gives Acme's count up to ten seconds to read $Expect. What
# it reads then is asserted after the rows are, so a pane that lost its rows fails on the rows.
function Switch-ReadState {
  param([Parameter(Mandatory)] [AllowEmptyString()] [string] $Expect)
  $toggle = Find-UiaElement -AutomationId 'SelectionMarkRead' -Type 'Button'
  if (-not $toggle) { throw 'the toolbar has no read toggle' }
  Invoke-UiaElement -Element $toggle | Out-Null
  for ($i = 0; $i -lt 40 -and (Get-AcmeUnread) -ne $Expect; $i++) { Start-Sleep -Milliseconds 250 }
}

$Suite = @{
  Dataset = 'harness'
  # Shares FolderDrop's launch.
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-imap' }
  Cases   = @(
    @{
      Name = 'an unread count that moves leaves the rows the pane shows where they were'
      Body = {
        $main = Get-MainWindow
        $saved = New-Object ReadWin.Input+RECT
        [void][ReadWin.Input]::GetWindowRect($main.Handle, [ref] $saved)
        $maximised = [Allodia.PaneWindow]::IsZoomed($main.Handle)
        try {
          [void][Allodia.PaneWindow]::MoveWindow($main.Handle, $saved.Left, $saved.Top,
            [int] (ConvertTo-UiaPixels 1200), [int] (ConvertTo-UiaPixels 560), $true)
          foreach ($folder in $Opened) { Set-FolderOpen -Folder $folder }
          Invoke-UiaElement -Element (Get-PaneFolderRow -Folder 'Acme') | Out-Null
          $message = @(Get-MailRows) | Where-Object { $_.Current.Name -like "*$Subject*" } | Select-Object -First 1
          if (-not $message) { throw "Acme does not show '$Subject'; was the harness reseeded?" }
          Invoke-UiaElement -Element $message | Out-Null

          $pane = Get-PaneScroll
          if (-not $pane) { throw 'the pane does not scroll at this window size' }
          $pane.Pattern.SetScrollPercent(-1, 100)
          Start-Sleep -Milliseconds 600
          $before = @(Get-ShownRows -Viewport $pane.Bounds)
          # Scrolled to its end, so a row it does not show is above the ones it does.
          Assert-True (@($before | Where-Object { $_ -like '2026@*' }).Count -eq 0 -and $before.Count -gt 2) `
            "Archive's open subfolders are above the rows the pane shows (it shows $($before -join ', '))"

          Switch-ReadState -Expect '1 unread'
          # A pane that rebuilt its rows opens its subfolders again a layout pass later; the shift
          # that leaves is what the reader sees, so it is read after that has had time to land.
          Start-Sleep -Seconds 2
          Assert-Equal ($before -join ', ') ((Get-ShownRows -Viewport $pane.Bounds) -join ', ') `
            'the pane shows the same rows in the same places after the count moved'
          Assert-Equal '1 unread' (Get-AcmeUnread) 'and the count it moved is the one Acme shows'

          Switch-ReadState -Expect ''
          Start-Sleep -Seconds 2
          Assert-Equal ($before -join ', ') ((Get-ShownRows -Viewport $pane.Bounds) -join ', ') `
            'and again after it moved back'
          Assert-Equal '' (Get-AcmeUnread) 'and Acme draws no count again'
        }
        finally {
          # A case that stopped between the two presses leaves the message unread.
          if ((Get-AcmeUnread) -eq '1 unread') { Switch-ReadState -Expect '' }
          # Innermost first: a shut parent takes its children's rows out of reach.
          foreach ($folder in @($Opened[-1..-($Opened.Count)])) { try { Set-FolderOpen -Folder $folder -Shut } catch { } }
          [void][Allodia.PaneWindow]::MoveWindow($main.Handle, $saved.Left, $saved.Top,
            $saved.Right - $saved.Left, $saved.Bottom - $saved.Top, $true)
          if ($maximised) { [void][Allodia.PaneWindow]::ShowWindow($main.Handle, 3) }   # SW_MAXIMIZE
        }
      }
    }
  )
}
