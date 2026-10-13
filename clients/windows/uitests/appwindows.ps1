#!/usr/bin/env pwsh
# Driving and reading this app's WINDOWS, for the suites that assert on more than one of them.
#
# Dot-source it from a suite:
#
#     . (Join-Path $PSScriptRoot 'appwindows.ps1')
#
# Separate from uia.ps1 because a second top-level window is a Win32 fact rather than a UI
# Automation one: which windows exist and in what order is EnumWindows, whether one carries an icon
# is WM_GETICON, who owns it is GetWindow. A double-click is a gesture no automation pattern can
# raise, so the presses go through winapp (winapp.ps1), like every other press in these suites.

Add-Type -Namespace ReadWin -Name Input -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
[DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int index);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
public struct RECT { public int Left, Top, Right, Bottom; }
[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
public delegate bool EnumProc(IntPtr h, IntPtr p);
'@

<#
.SYNOPSIS
Every visible top-level window of the running app, as @{ Handle; Title }.
.DESCRIPTION
Enumerated rather than read off the process: Process.MainWindowHandle names ONE window, so it can
neither see a reading window nor tell you how many are open, which is most of what this suite asks.
#>
function Get-AppWindows {
  $procIds = @((Get-Process Mailcal -ErrorAction SilentlyContinue).Id)
  $found = New-Object System.Collections.ArrayList
  $callback = [ReadWin.Input+EnumProc] {
    param($handle, $param)
    $owner = 0
    [void][ReadWin.Input]::GetWindowThreadProcessId($handle, [ref] $owner)
    if ($procIds -contains $owner -and [ReadWin.Input]::IsWindowVisible($handle)) {
      $text = New-Object System.Text.StringBuilder 512
      [void][ReadWin.Input]::GetWindowTextW($handle, $text, 512)
      if ($text.Length -gt 0) {
        [void]$found.Add([pscustomobject]@{ Handle = $handle; Title = $text.ToString() })
      }
    }
    return $true
  }
  [void][ReadWin.Input]::EnumWindows($callback, [IntPtr]::Zero)
  $found
}

<#
.SYNOPSIS
Raises the mailbox the way a person does, by pressing its title bar.
.DESCRIPTION
SetForegroundWindow alone is not enough any more, and should not be: a window this app has just
opened stays in front until the reader asks for the mailbox, and a press is what the app accepts as
asking. The title bar because it is the part of the mailbox a message window never covers, and
because a press there reaches no control.
#>
function Show-Mailbox {
  $window = Get-MailcalWindow
  if (-not $window) { throw 'the mailbox window is not open' }
  # The app's NAME in the title bar, well left of the search box that sits in the middle of it:
  # pressing that opens search and takes the message list off screen, which reads as the mailbox
  # having no list at all.
  $title = Find-UiaElement -Root $window -Name (Get-BrandAppTitle) -Type 'Text'
  if (-not $title) { throw "the mailbox's title bar shows no '$(Get-BrandAppTitle)'" }
  Invoke-WinappClick $title
  Start-Sleep -Milliseconds 300
}

function Get-MainWindow {
  Get-AppWindows | Where-Object { $_.Title -eq (Get-BrandAppTitle) } | Select-Object -First 1
}

function Get-ExtraWindows {
  $main = Get-BrandAppTitle
  # PopupHost is the top-level window XAML puts a flyout in, so an open context menu counts as one
  # of the app's windows unless it is named out. It lingers for a moment after the menu is invoked,
  # which is long enough to be counted and to fail an assertion about how many windows opened.
  @(Get-AppWindows | Where-Object { $_.Title -ne $main -and $_.Title -ne 'PopupHost' })
}

<#
.SYNOPSIS
Wait for a window titled -Title, and return its automation root.
#>
function Wait-AppWindow {
  param([Parameter(Mandatory)] [string] $Title, [int] $TimeoutSec = 20)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    if (Get-AppWindows | Where-Object { $_.Title -eq $Title } | Select-Object -First 1) {
      $window = Get-AppWindows | Where-Object { $_.Title -eq $Title } | Select-Object -First 1
      return [System.Windows.Automation.AutomationElement]::FromHandle($window.Handle)
    }
    Start-Sleep -Milliseconds 250
  }
  throw "no window titled '$Title' within ${TimeoutSec}s; open windows: $((Get-AppWindows).Title -join ' | ')"
}

<#
.SYNOPSIS
Close every window but the mailbox, and wait until they are gone.
.DESCRIPTION
WM_CLOSE rather than the window's own Close button. Every window here has one, but it belongs to
the system rather than to the app (Services/WindowCaption.cs), so pressing it is an OS gesture on a
surface outside the XAML tree, and a case that is closing up after itself should not depend on one.
A close that silently does nothing leaves the window open and hands the next case somebody else's
windows to count.

It also matters that the windows really go: uia.ps1 resolves the app through
Process.MainWindowHandle, which names ONE window, so a leftover reading window can shadow the
mailbox and every row lookup then reports that the list is not on screen.
#>
function Close-ExtraWindows {
  foreach ($window in Get-ExtraWindows) {
    [void][ReadWin.Input]::SendMessageW($window.Handle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
  }
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ((Get-ExtraWindows).Count -gt 0 -and $timer.Elapsed.TotalSeconds -lt 10) {
    Start-Sleep -Milliseconds 200
  }
  if ((Get-ExtraWindows).Count -gt 0) {
    throw "windows would not close: $((Get-ExtraWindows).Title -join ' | ')"
  }
}

<#
.SYNOPSIS
Click the row whose title is -Subject, once, or twice as a double-click.
.DESCRIPTION
The app is brought forward first: a press goes to whatever is under the pointer, so without that it
lands on whichever window the previous case left in front.
#>
function Invoke-RowClicks {
  param([Parameter(Mandatory)] [string] $Subject, [ValidateSet(1, 2)] [int] $Times = 1)
  Show-Mailbox
  Invoke-WinappClick (Get-MailRowByTitle $Subject) -Double:($Times -eq 2)
  # The window is opened on the double-tap and the pane is corrected after the trailing click, so
  # settle past both before reading anything.
  Start-Sleep -Milliseconds 1500
}

<#
.SYNOPSIS
Click once at a screen point in the mailbox window.
.DESCRIPTION
For the composer's message body. Everywhere else a press is aimed at an element; while a composer
is open winapp cannot look one up (winapp.ps1 says why), and the body is a web page whose parts UI
Automation does not see anyway, so this one is aimed at a point.
#>
function Invoke-ClicksAt {
  param([Parameter(Mandatory)] [int] $X, [Parameter(Mandatory)] [int] $Y)
  $main = Get-MainWindow
  if (-not $main) { throw 'the mailbox window is not open' }
  Invoke-WinappPress -Point $X, $Y -Window ([long] $main.Handle)
  Start-Sleep -Milliseconds 1500
}

<#
.SYNOPSIS
Raise the context menu on the row whose title is -Subject.
.DESCRIPTION
A real right-click rather than Shift+F10. The keyboard shortcut goes to whatever the ListView has
focused, which is the item container; the flyout is declared on the row TEMPLATE's own Grid, so the
keypress reaches nothing at all and the case fails for a reason that has nothing to do with the
item under test.
#>
function Show-RowContextMenu {
  param([Parameter(Mandatory)] [string] $Subject)
  Show-Mailbox
  Invoke-WinappClick (Get-MailRowByTitle $Subject) -Right
  Start-Sleep -Milliseconds 600
}

<#
.SYNOPSIS
Whether -Handle's window carries an icon of its own.
.DESCRIPTION
WM_GETICON, because that is what the title bar, the taskbar and Alt-Tab read. WinUI 3 does not put
the exe's embedded icon on a window by itself, so every window this app opens has to ask for it,
and a window added later that forgets is invisible to every other assertion here: it draws, it
responds, and it simply wears the system default beside its siblings.
#>
function Test-WindowIcon {
  param([Parameter(Mandatory)] $Handle)
  $small = [ReadWin.Input]::SendMessageW($Handle, 0x7F, [IntPtr] 0, [IntPtr] 0)   # WM_GETICON, SMALL
  $big = [ReadWin.Input]::SendMessageW($Handle, 0x7F, [IntPtr] 1, [IntPtr] 0)     # WM_GETICON, BIG
  $small -ne [IntPtr]::Zero -and $big -ne [IntPtr]::Zero
}

<#
.SYNOPSIS
The title of whichever window currently holds the foreground.
#>
function Get-ForegroundTitle {
  $text = New-Object System.Text.StringBuilder 512
  [void][ReadWin.Input]::GetWindowTextW([ReadWin.Input]::GetForegroundWindow(), $text, 512)
  $text.ToString()
}
