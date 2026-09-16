#!/usr/bin/env pwsh
# Driving and reading this app's WINDOWS, for the suites that assert on more than one of them.
#
# Dot-source it from a suite:
#
#     . (Join-Path $PSScriptRoot 'appwindows.ps1')
#
# Separate from uia.ps1 because UI Automation is not what any of this uses. A second top-level
# window is a Win32 fact: which windows exist and in what order is EnumWindows, whether one carries
# an icon is WM_GETICON, who owns it is GetWindow, and a double-click is a gesture no automation
# pattern can raise, so the presses are injected with mouse_event. Nothing here needs a UIA tree.

Add-Type -Namespace ReadWin -Name Input -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int data, UIntPtr extra);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
[DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int index);
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
WM_CLOSE rather than a Close button. The shell draws its own caption (MainWindow.TitleBar.cs) and
so has a #Close element; a reading or composer window carries the SYSTEM caption, whose buttons are
not in the app's automation tree at all. Invoking a #Close that is not there fails silently, which
leaves the window open and hands the next case somebody else's windows to count.

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
Click the row whose title is -Subject, -Times times in quick succession.
.DESCRIPTION
The presses are 60ms apart, comfortably inside any double-click setting, and the app is brought
forward first: injected input goes to whatever has focus, so without that the clicks land on
whichever window the previous case left in front.
#>
function Invoke-RowClicks {
  param([Parameter(Mandatory)] [string] $Subject, [int] $Times = 1)
  $main = Get-MainWindow
  if (-not $main) { throw 'the mailbox window is not open' }
  [void][ReadWin.Input]::SetForegroundWindow($main.Handle)
  Start-Sleep -Milliseconds 300

  $row = Get-MailRowByTitle $Subject
  $bounds = $row.Current.BoundingRectangle
  $x = [int] ($bounds.X + $bounds.Width / 2)
  $y = [int] ($bounds.Y + $bounds.Height / 2)
  [void][ReadWin.Input]::SetCursorPos($x, $y)
  Start-Sleep -Milliseconds 120
  foreach ($i in 1..$Times) {
    [ReadWin.Input]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)   # LEFTDOWN
    [ReadWin.Input]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)   # LEFTUP
    if ($i -lt $Times) { Start-Sleep -Milliseconds 60 }
  }
  # The window is opened on the double-tap and the pane is corrected after the trailing click, so
  # settle past both before reading anything.
  Start-Sleep -Milliseconds 1500
}

<#
.SYNOPSIS
Click -Times times at a screen point, with the mailbox window in front.
#>
function Invoke-ClicksAt {
  param([Parameter(Mandatory)] [int] $X, [Parameter(Mandatory)] [int] $Y, [int] $Times = 1)
  $main = Get-MainWindow
  if (-not $main) { throw 'the mailbox window is not open' }
  [void][ReadWin.Input]::SetForegroundWindow($main.Handle)
  Start-Sleep -Milliseconds 300
  [void][ReadWin.Input]::SetCursorPos($X, $Y)
  Start-Sleep -Milliseconds 120
  foreach ($i in 1..$Times) {
    [ReadWin.Input]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)   # LEFTDOWN
    [ReadWin.Input]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)   # LEFTUP
    if ($i -lt $Times) { Start-Sleep -Milliseconds 60 }
  }
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
  $main = Get-MainWindow
  if (-not $main) { throw 'the mailbox window is not open' }
  [void][ReadWin.Input]::SetForegroundWindow($main.Handle)
  Start-Sleep -Milliseconds 300
  $row = Get-MailRowByTitle $Subject
  $bounds = $row.Current.BoundingRectangle
  [void][ReadWin.Input]::SetCursorPos(
    [int] ($bounds.X + $bounds.Width / 2), [int] ($bounds.Y + $bounds.Height / 2))
  Start-Sleep -Milliseconds 120
  [ReadWin.Input]::mouse_event(0x0008, 0, 0, 0, [UIntPtr]::Zero)   # RIGHTDOWN
  [ReadWin.Input]::mouse_event(0x0010, 0, 0, 0, [UIntPtr]::Zero)   # RIGHTUP
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
