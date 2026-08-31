#!/usr/bin/env pwsh
# Synthetic TOUCH for the WinUI client, real contacts, not a mouse pretending.
#
# This exists because the repo believed it was impossible. `docs/debugging.md` and the
# `verify-windows-ui` skill both said a WinUI gesture "needs real touch/pen/precision-touchpad
# input, which cannot be synthesized", and the MAILCAL_SWIPE launch hook was built to route around
# that wall. **The wall is not there.** Win32's InitializeTouchInjection/InjectTouchInput inject at
# the pointer-device level: the OS delivers them as genuine touch, so they drive the real gesture
# pipeline, SwipeControl, ScrollView, and the calendar's own pointer owner, from an ordinary,
# unelevated process. No capability, no package identity, no elevation, no Store approval.
#
# (The WinRT `Windows.UI.Input.Preview.Injection.InputInjector` is the one that needs the
# `inputInjectionBrokered` restricted capability AND package identity, which the unpackaged dev
# loop does not have. That is almost certainly where the "impossible" came from. Use this instead.)
#
# WHAT THIS IS AND IS NOT GOOD FOR
#
#   Good: proving a gesture is wired up end-to-end, that a swipe reaches the grid, that two
#   contacts are delivered together and read as a pinch, that the week turns. Also: generating
#   sustained, repeatable MOTION so the frame budget can be measured during it (docs/calendar.md §7).
#
#   Not good: the bugs that actually matter. A script cannot land a flick exactly one frame into the
#   previous turn's animation, and that race is where the swallowed swipe lived. **Every test that
#   waits for the grid to settle first is testing the case that already worked** (§9). That race is
#   covered instead by Mailcal.Tests/CalendarFlickTests.cs, which owns the clock. Use both; neither
#   replaces the other.
#
# Dot-source it:
#
#   . ./clients/windows/touch.ps1
#   Initialise-Touch
#   $w = Get-MailcalBounds
#   Invoke-TouchFlick -FromX ($w.Right - 120) -ToX ($w.Left + 120) -Y $w.MidY   # turn the week
#   Invoke-TouchPinch -CenterX $w.MidX -CenterY $w.MidY -FromSpread 120 -ToSpread 320
#
# THE FIVE TRAPS, all of which cost real time to find:
#   1. POINTER_FLAG_NEW on the DOWN frame -> ERROR_INVALID_PARAMETER (87). The Win32 path sanctions
#      only INRANGE|INCONTACT|DOWN. (The WinRT sample DOES use New, copying it here fails.)
#   2. Coordinates are PHYSICAL screen pixels on the virtual desktop. The injector must be
#      per-monitor DPI-aware or every point lands somewhere else under display scaling.
#   3. The UP frame must be at the SAME point as the last UPDATE, or the injection fails and every
#      active contact is cancelled.
#   4. A press-and-hold needs repeated UPDATE frames. Stop sending and the contact is cancelled.
#   5. It is a real finger: input goes to whatever window is under the point. The target must be
#      foreground and unoccluded, so assert what is on screen before injecting, never after.

# NB: deliberately no `Set-StrictMode` here. This file is DOT-SOURCED, so anything it sets leaks into
# the caller's session, and strict mode turns their perfectly normal unset `$LASTEXITCODE` into a
# hard error. A helper does not get to change the rules of the shell that loaded it.

if (-not ('Allodia.Touch' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace Allodia
{
    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int x; public int y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int left; public int top; public int right; public int bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINTER_INFO
    {
        public uint pointerType;
        public uint pointerId;
        public uint frameId;
        public uint pointerFlags;
        public IntPtr sourceDevice;
        public IntPtr hwndTarget;
        public POINT ptPixelLocation;
        public POINT ptHimetricLocation;
        public POINT ptPixelLocationRaw;
        public POINT ptHimetricLocationRaw;
        public uint dwTime;
        public uint historyCount;
        public int inputData;
        public uint dwKeyStates;
        public ulong performanceCount;
        public int buttonChangeType;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINTER_TOUCH_INFO
    {
        public POINTER_INFO pointerInfo;
        public uint touchFlags;
        public uint touchMask;
        public RECT rcContact;
        public RECT rcContactRaw;
        public uint orientation;
        public uint pressure;
    }

    /// <summary>Window lookup, in the SAME assembly as RECT, a second Add-Type block cannot see it.</summary>
    public static class Win
    {
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    }

    public static class Touch
    {
        const uint PT_TOUCH = 2;

        const uint POINTER_FLAG_INRANGE   = 0x00000002;
        const uint POINTER_FLAG_INCONTACT = 0x00000004;
        const uint POINTER_FLAG_DOWN      = 0x00010000;
        const uint POINTER_FLAG_UPDATE    = 0x00020000;
        const uint POINTER_FLAG_UP        = 0x00040000;

        const uint TOUCH_MASK_CONTACTAREA = 0x00000001;
        const uint TOUCH_MASK_PRESSURE    = 0x00000004;

        // Trap 1: DOWN is INRANGE|INCONTACT|DOWN. Adding POINTER_FLAG_NEW gives error 87.
        public const uint Down   = POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_DOWN;
        public const uint Update = POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_UPDATE;
        public const uint Up     = POINTER_FLAG_UP;

        [DllImport("user32.dll", SetLastError = true)]
        static extern bool InitializeTouchInjection(uint maxCount, uint dwMode);

        [DllImport("user32.dll", SetLastError = true)]
        static extern bool InjectTouchInput(uint count, [In] POINTER_TOUCH_INFO[] contacts);

        [DllImport("user32.dll", SetLastError = true)]
        static extern bool SetProcessDpiAwarenessContext(IntPtr value);

        // Trap 2: without this, every injected point lands in the wrong place under scaling.
        static readonly IntPtr PER_MONITOR_AWARE_V2 = new IntPtr(-4);

        public static void Initialize(uint maxContacts)
        {
            SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2);   // best-effort; may already be set
            const uint TOUCH_FEEDBACK_INDIRECT = 2;                 // draw the visual, don't hit-test it
            if (!InitializeTouchInjection(maxContacts, TOUCH_FEEDBACK_INDIRECT))
            {
                throw new InvalidOperationException(
                    "InitializeTouchInjection failed: " + Marshal.GetLastWin32Error());
            }
        }

        public static POINTER_TOUCH_INFO Contact(uint id, int x, int y, uint flags)
        {
            var c = new POINTER_TOUCH_INFO();
            c.pointerInfo.pointerType = PT_TOUCH;
            c.pointerInfo.pointerId = id;
            c.pointerInfo.pointerFlags = flags;
            c.pointerInfo.ptPixelLocation.x = x;
            c.pointerInfo.ptPixelLocation.y = y;
            c.touchFlags = 0;
            c.touchMask = TOUCH_MASK_CONTACTAREA | TOUCH_MASK_PRESSURE;
            c.pressure = 1024;
            // A finger has an area. Some hit-testing paths care.
            c.rcContact.left = x - 4; c.rcContact.right = x + 4;
            c.rcContact.top = y - 4;  c.rcContact.bottom = y + 4;
            return c;
        }

        /// <summary>One frame of contacts. Throws with the Win32 code, which is the only useful
        /// diagnostic this API gives you.</summary>
        public static void Send(POINTER_TOUCH_INFO[] contacts)
        {
            if (!InjectTouchInput((uint)contacts.Length, contacts))
            {
                int err = Marshal.GetLastWin32Error();
                // 87 = ERROR_INVALID_PARAMETER: almost always a bad flag combination, an off-screen
                // point, or an UP that moved. 21 = ERROR_NOT_READY: frames came too fast; resend.
                throw new InvalidOperationException("InjectTouchInput failed: " + err);
            }
        }
    }
}
'@
}

<#
.SYNOPSIS
  Arms touch injection. Call once per process.
#>
function Initialize-Touch {
  param([uint32] $MaxContacts = 4)
  [Allodia.Touch]::Initialize($MaxContacts)
}

# Frames closer together than ~0.1ms return ERROR_NOT_READY (trap 4's cousin). A real finger reports
# at ~100-240Hz; 8ms is a comfortable, realistic cadence that never trips it.
$script:FrameMs = 8

function Send-TouchFrame {
  param([Parameter(Mandatory)] [object[]] $Contacts)
  [Allodia.Touch]::Send([Allodia.POINTER_TOUCH_INFO[]] $Contacts)
  Start-Sleep -Milliseconds $script:FrameMs
}

<#
.SYNOPSIS
  A one-finger drag. -DurationMs short + a long distance is a FLICK (pages the week on velocity);
  long + short is a slow drag (pages only past the threshold).
#>
function Invoke-TouchDrag {
  param(
    [Parameter(Mandatory)] [int] $FromX,
    [Parameter(Mandatory)] [int] $FromY,
    [Parameter(Mandatory)] [int] $ToX,
    [Parameter(Mandatory)] [int] $ToY,
    [int] $DurationMs = 250,
    [uint32] $Id = 1
  )
  $steps = [Math]::Max(2, [int]($DurationMs / $script:FrameMs))
  Send-TouchFrame @([Allodia.Touch]::Contact($Id, $FromX, $FromY, [Allodia.Touch]::Down))
  for ($i = 1; $i -le $steps; $i++) {
    $x = [int]($FromX + ($ToX - $FromX) * $i / $steps)
    $y = [int]($FromY + ($ToY - $FromY) * $i / $steps)
    Send-TouchFrame @([Allodia.Touch]::Contact($Id, $x, $y, [Allodia.Touch]::Update))
  }
  # Trap 3: the UP must be exactly where the last UPDATE was.
  Send-TouchFrame @([Allodia.Touch]::Contact($Id, $ToX, $ToY, [Allodia.Touch]::Up))
}

<#
.SYNOPSIS
  A fast horizontal flick, the gesture nearly every real page turn is made of.
#>
function Invoke-TouchFlick {
  param(
    [Parameter(Mandatory)] [int] $FromX,
    [Parameter(Mandatory)] [int] $ToX,
    [Parameter(Mandatory)] [int] $Y,
    [int] $DurationMs = 60
  )
  Invoke-TouchDrag -FromX $FromX -FromY $Y -ToX $ToX -ToY $Y -DurationMs $DurationMs
}

<#
.SYNOPSIS
  A two-finger pinch. Spread along both axes to exercise the DIAGONAL zoom (docs/calendar.md §8),
  that is the one a touchpad cannot do, so it can only be tested here.
.PARAMETER AngleDeg
  0 = purely horizontal (days only). 90 = purely vertical (hours only). 45 = diagonal (both).
#>
function Invoke-TouchPinch {
  param(
    [Parameter(Mandatory)] [int] $CenterX,
    [Parameter(Mandatory)] [int] $CenterY,
    [Parameter(Mandatory)] [int] $FromSpread,
    [Parameter(Mandatory)] [int] $ToSpread,
    [double] $AngleDeg = 45,
    [int] $DurationMs = 400
  )
  $rad = $AngleDeg * [Math]::PI / 180
  $ux = [Math]::Cos($rad)
  $uy = [Math]::Sin($rad)
  $steps = [Math]::Max(2, [int]($DurationMs / $script:FrameMs))

  function Pair([int] $spread) {
    $hx = [int]($ux * $spread / 2)
    $hy = [int]($uy * $spread / 2)
    return @(
      @{ X = $CenterX - $hx; Y = $CenterY - $hy },
      @{ X = $CenterX + $hx; Y = $CenterY + $hy }
    )
  }

  $p = Pair $FromSpread
  Send-TouchFrame @(
    [Allodia.Touch]::Contact(1, $p[0].X, $p[0].Y, [Allodia.Touch]::Down),
    [Allodia.Touch]::Contact(2, $p[1].X, $p[1].Y, [Allodia.Touch]::Down)
  )
  for ($i = 1; $i -le $steps; $i++) {
    $spread = [int]($FromSpread + ($ToSpread - $FromSpread) * $i / $steps)
    $p = Pair $spread
    Send-TouchFrame @(
      [Allodia.Touch]::Contact(1, $p[0].X, $p[0].Y, [Allodia.Touch]::Update),
      [Allodia.Touch]::Contact(2, $p[1].X, $p[1].Y, [Allodia.Touch]::Update)
    )
  }
  $p = Pair $ToSpread
  Send-TouchFrame @(
    [Allodia.Touch]::Contact(1, $p[0].X, $p[0].Y, [Allodia.Touch]::Up),
    [Allodia.Touch]::Contact(2, $p[1].X, $p[1].Y, [Allodia.Touch]::Up)
  )
}

<#
.SYNOPSIS
  The running client's window rectangle, in the physical screen pixels touch injection speaks.
.DESCRIPTION
  Trap 5: injected touch goes to whatever window is under the point, exactly as a finger would. This
  brings the client to the foreground and hands back its bounds so a caller cannot inject into
  whatever happened to be on top.
#>
function Get-MailcalBounds {
  $p = Get-Process -Name 'Mailcal' -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $p) { throw 'Mailcal is not running.' }

  $h = $p.MainWindowHandle
  if ($h -eq [IntPtr]::Zero) { throw 'Mailcal has no main window yet.' }
  [void][Allodia.Win]::SetForegroundWindow($h)
  Start-Sleep -Milliseconds 300

  $r = New-Object Allodia.RECT
  [void][Allodia.Win]::GetWindowRect($h, [ref] $r)

  # An empty rect means the lookup failed, and injecting against it puts every contact off-screen,
  # which surfaces as a baffling ERROR_INVALID_PARAMETER (87) from InjectTouchInput rather than as
  # the window-lookup bug it actually is. Fail here, where the cause is legible.
  if ($r.right -le $r.left -or $r.bottom -le $r.top) {
    throw "GetWindowRect gave an empty rect ($($r.left),$($r.top))-($($r.right),$($r.bottom))."
  }

  [pscustomobject]@{
    Left   = $r.left
    Top    = $r.top
    Right  = $r.right
    Bottom = $r.bottom
    MidX   = [int](($r.left + $r.right) / 2)
    MidY   = [int](($r.top + $r.bottom) / 2)
  }
}
