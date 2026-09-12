#!/usr/bin/env pwsh
# Sets the desktop resolution, for a host whose default is too small for the UI suite to measure
# anything. The suites drive a 1440x900 logical window (run-ui-tests.ps1 ->
# Assert-DesktopFitsShowcase) and a GitHub Windows runner comes up at 1024x768.
#
#   ./set-desktop-resolution.ps1                    # 1920x1080
#   ./set-desktop-resolution.ps1 -Width 2560 -Height 1440
#
# CI-shaped rather than developer-shaped: nobody resizes their own desktop to run a test. It is a
# script rather than inline in the workflow so a failure is debuggable by hand, and so the number
# the suite needs and the number CI sets cannot drift into two files.
#
# ⚠️ IT REPORTS WHAT IT GOT, not what it asked for, and fails when they differ.
# `ChangeDisplaySettingsEx` returns DISP_CHANGE_SUCCESSFUL for a mode a virtual adapter then does
# not apply, and a caller that believes a 1024x768 desktop is 1920x1080 goes on to sample screen
# regions past the edge, which come back BLACK and read as a theme bug.
#
# ⚠️ THE INTEROP LIVES IN C#, not in PowerShell. `EnumDisplaySettings` takes DEVMODE by reference,
# and calling it with a `[ref]` to a struct PowerShell made returns FALSE with no last error: the
# two `ByValTStr` fields are null in a fresh struct and the marshaller will not write them. Doing
# the whole call inside the compiled type sidesteps it. The tell is a probe where
# `GetSystemMetrics` answers correctly on the same line that `EnumDisplaySettings` fails.
[CmdletBinding()]
param(
  [int] $Width = 1920,
  [int] $Height = 1080
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'this only means anything on Windows.' }

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
internal struct DEVMODEW
{
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
    public ushort dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra;
    public uint dmFields;
    // The display half of DEVMODE's union: POINTL dmPosition, then the two orientation fields.
    // Sixteen bytes either way, so the printer half's eight shorts would marshal to the same
    // size and the wrong meaning.
    public int dmPositionX, dmPositionY;
    public uint dmDisplayOrientation, dmDisplayFixedOutput;
    public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
    public ushort dmLogPixels;
    public uint dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
    public uint dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2;
    public uint dmPanningWidth, dmPanningHeight;
}

public static class MailcalDesktop
{
    private const int EnumCurrentSettings = -1;
    private const uint FieldPelsWidth = 0x00080000;
    private const uint FieldPelsHeight = 0x00100000;

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool EnumDisplaySettingsW(string deviceName, int modeNum, ref DEVMODEW devMode);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int ChangeDisplaySettingsExW(
        string deviceName, ref DEVMODEW devMode, IntPtr window, uint flags, IntPtr param);

    /// <summary>The desktop's current size in physical pixels, or null when it cannot be read.</summary>
    public static int[] Current()
    {
        var mode = new DEVMODEW();
        mode.dmSize = (ushort) Marshal.SizeOf(typeof(DEVMODEW));
        if (!EnumDisplaySettingsW(null, EnumCurrentSettings, ref mode))
        {
            return null;
        }
        return new[] { (int) mode.dmPelsWidth, (int) mode.dmPelsHeight };
    }

    /// <summary>DISP_CHANGE_*, 0 being success. Changes the two size fields and nothing else, so
    /// the colour depth and refresh rate stay whatever the adapter is happy with.</summary>
    public static int Resize(int width, int height)
    {
        var mode = new DEVMODEW();
        mode.dmSize = (ushort) Marshal.SizeOf(typeof(DEVMODEW));
        if (!EnumDisplaySettingsW(null, EnumCurrentSettings, ref mode))
        {
            return int.MinValue;
        }
        mode.dmPelsWidth = (uint) width;
        mode.dmPelsHeight = (uint) height;
        mode.dmFields = FieldPelsWidth | FieldPelsHeight;
        return ChangeDisplaySettingsExW(null, ref mode, IntPtr.Zero, 0, IntPtr.Zero);
    }
}
'@

$before = [MailcalDesktop]::Current()
if (-not $before) { throw 'could not read the current display mode; is there a desktop session at all?' }
Write-Host "==> desktop is $($before[0])x$($before[1])"
if ($before[0] -eq $Width -and $before[1] -eq $Height) {
  Write-Host '==> already the size asked for'
  exit 0
}

$result = [MailcalDesktop]::Resize($Width, $Height)
if ($result -ne 0) {
  throw "ChangeDisplaySettingsEx refused ${Width}x${Height} (DISP_CHANGE $result). Ask for a mode the adapter lists."
}

# Polled, never slept: the mode change is asynchronous and a virtual adapter takes a moment to
# re-initialise, so an immediate read can still report the old size on a change that did work.
$timer = [Diagnostics.Stopwatch]::StartNew()
while ($timer.Elapsed.TotalSeconds -lt 15) {
  $now = [MailcalDesktop]::Current()
  if ($now -and $now[0] -eq $Width -and $now[1] -eq $Height) {
    Write-Host "==> desktop is now $($now[0])x$($now[1])"
    exit 0
  }
  Start-Sleep -Milliseconds 250
}
$last = [MailcalDesktop]::Current()
throw "the call succeeded but the desktop is still $($last[0])x$($last[1]), not ${Width}x${Height}."
