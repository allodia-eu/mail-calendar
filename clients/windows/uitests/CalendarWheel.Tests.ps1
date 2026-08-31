#!/usr/bin/env pwsh
# Does a wheel actually move the calendar? The one question about the grid's scrolling that no
# headless test in this repo can answer.
#
# `Mailcal.Tests` links no WinUI, so it exercises the gesture owner, the driver and the state machine
# directly, it never sees `CalendarSurface`, which is where the wheel event is received, handed to
# the owner, and turned into a drawn frame. Every defect this suite exists for lived in exactly that
# gap and left the pure layer's tests green:
#
#   * `OnPointerWheel` started the tick loop without marking the surface dirty, so a notch arriving
#     with nothing else moving started a loop that then drew nothing.
#   * The loop seeded its clock on the first callback rather than at start, spending a frame on every
#     restart, and a wheel restarts it per notch.
#   * The wheel's travel was applied to the strip instantly, so the grid teleported once per notch
#     and stood still in between. On a real mouse at ~150 ms between notches that measured 16-31 fps
#     of a 60 fps budget, at a mean frame cost of 6.6 ms: the frames were never slow, they were never
#     asked for.
#
# **UIA cannot deliver a wheel**, it drives control patterns, not pointers (see uia.ps1's header).
# So the input here is injected at the OS level with `mouse_event`, the same way `touch.ps1` injects
# real touch. That means the window has to be foregrounded and the cursor parked over the grid first,
# or the notches land on whatever is actually under the pointer and the suite passes having tested
# nothing.
#
# The oracle is the period heading (`CalendarPeriod`), which names the span the grid is showing. It is
# coarse, it cannot see a single day of travel, but it is the only position the grid publishes, and
# it is enough for the two rules that matter: the wheel moves the grid at all, and it moves it by an
# amount that depends on how far you scrolled rather than by a fixed page.
#
# Dataset is `showcase`: seeded, offline, no real account, and no mail action is dispatched.

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CalWheelInput {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int data, UIntPtr extra);
  public const uint HWHEEL = 0x01000;
}
'@ -ErrorAction SilentlyContinue

# Parks the cursor in the middle of the time grid and delivers horizontal notches at a mouse's own
# cadence. Returns nothing; read the period afterwards.
function Invoke-GridWheel {
  param([int]$Notches, [int]$GapMs = 120)

  $app = Get-Process Mailcal -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $app) { throw 'the app is not running' }
  [CalWheelInput]::SetForegroundWindow($app.MainWindowHandle) | Out-Null
  Start-Sleep -Milliseconds 500

  # The middle of the grid, derived from the header buttons rather than hard-coded: they sit at the
  # grid's top-right, so their row gives the content's left edge and the window's width.
  $next = Get-UiaTree | Where-Object { $_.Current.AutomationId -eq 'CalendarNext' } | Select-Object -First 1
  if (-not $next) { throw 'the calendar is not on screen' }
  $r = $next.Current.BoundingRectangle
  $x = [int]($r.X - 200)              # left of the chevrons, inside the day columns
  $y = [int]($r.Y + $r.Height + 400)  # well below the header, into the hour grid
  [CalWheelInput]::SetCursorPos($x, $y) | Out-Null
  Start-Sleep -Milliseconds 300

  for ($i = 0; $i -lt $Notches; $i++) {
    [CalWheelInput]::mouse_event([CalWheelInput]::HWHEEL, 0, 0, 120, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds $GapMs
  }
  # Longer than the pan's own idle window, so the grid has come to rest before anything is read.
  Start-Sleep -Milliseconds 600
}

# Brings the calendar up. NOT a launch hook: the showcase dataset is started through showcase.ps1,
# which pins the screen it opens on, so MAILCAL_CALENDAR never reaches this process. Clicking the nav
# item is what a user does anyway.
function Show-Calendar {
  $nav = Get-UiaTree | Where-Object { $_.Current.AutomationId -eq 'NavCalendar' } | Select-Object -First 1
  if (-not $nav) { throw 'the Calendar navigation item is not on screen' }
  Invoke-UiaElement $nav
  if (-not (Wait-UiaElement -AutomationId 'CalendarPeriod' -TimeoutSec 30)) {
    throw 'the calendar never came up'
  }
  Start-Sleep -Milliseconds 1200   # let the first pages paint before anything is measured
}

function Get-CalendarPeriod {
  (Get-UiaTree |
    Where-Object { $_.Current.AutomationId -eq 'CalendarPeriod' } |
    Select-Object -First 1).Current.Name
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'a horizontal wheel moves the grid'
      Body = {
        Show-Calendar
        $before = Get-CalendarPeriod
        Assert-True ([bool]$before) 'the calendar names the span it is showing'

        Invoke-GridWheel -Notches 24

        $after = Get-CalendarPeriod
        Assert-True ($after -ne $before) (
          "the grid still says '$after' after 24 wheel notches. A wheel that moves nothing is the " +
          'defect this suite exists for, and it is invisible to every headless gate: the owner and ' +
          'the driver are perfectly happy, the surface simply never draws.')
      }
    },
    @{
      Name = 'scrolling further travels further, rather than turning one page'
      Body = {
        Show-Calendar
        # Home first, so both legs start from the same place.
        $today = Get-UiaTree | Where-Object { $_.Current.AutomationId -eq 'CalendarToday' } | Select-Object -First 1
        Invoke-UiaElement $today
        Start-Sleep -Milliseconds 800
        $origin = Get-CalendarPeriod

        Invoke-GridWheel -Notches 12
        $short = Get-CalendarPeriod

        Invoke-UiaElement $today
        Start-Sleep -Milliseconds 800
        Assert-Equal $origin (Get-CalendarPeriod) 'Today puts the grid back where it started'

        Invoke-GridWheel -Notches 48
        $long = Get-CalendarPeriod

        # The grid is a continuous strip, not a pager: four times the input goes four times as far,
        # so the two legs cannot land on the same span. A page turn per gesture would.
        Assert-True ($long -ne $short) (
          "12 notches and 48 notches both landed on '$short'. The grid is paging by a fixed span " +
          'instead of travelling by what the wheel actually asked for.')
      }
    }
  )
}
