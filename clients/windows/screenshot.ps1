#!/usr/bin/env pwsh
# Capture a PNG of the running WinUI client (Mailcal), the Windows counterpart of the
# simulator/adb screenshots in scripts/dev/screenshot.sh, which shells out to this on a Windows
# host. Grabs the app window itself (not the whole screen) via PrintWindow with
# PW_RENDERFULLCONTENT, the flag needed for a WinUI/DirectComposition surface; falls back to a
# screen-region BitBlt if PrintWindow can't render the frame. Prints the saved path on the last
# line so the caller can capture it.
#
#   ./screenshot.ps1                       # -> $env:TEMP\mailcal-windows.png
#   ./screenshot.ps1 -Out C:\tmp\shot.png
#
# ---------------------------------------------------------------------------------------------
# Why the unpainted margin is cropped off, and then asserted gone:
#
# GetWindowRect does NOT return what you see. Since Vista, a sizeable window's rect is inflated by
# an *invisible* resize border, SM_CXSIZEFRAME + SM_CXPADDEDBORDER, 13 physical px at 200%, none
# at the top on a restored window, that exists only to give the mouse something to grab. The app
# paints none of it, so PrintWindow leaves it at the bitmap's zero-fill and every capture came out
# framed in a black L: 13 px down the left, the right, and the bottom. It is not a rendering bug in
# the app and no amount of settling makes it go away; it shipped into the committed store set.
#
# The obvious fix, crop to DWMWA_EXTENDED_FRAME_BOUNDS, is 2 px short on every side, because DWM
# draws the window's visible border *itself*: PrintWindow asks the app to render, and the app does
# not own those pixels, so they come back black too. So measure the unpainted margin instead, and
# cap the crop at the frame inset the metrics report. That also covers a *maximised* window, whose
# rect is inflated at the top as well.
#
# The black edge is then asserted gone rather than assumed gone. This is exactly the failure this
# script produces silently: a perfectly valid, healthy-sized PNG of the right app, which showcase.sh's
# byte floor waves through and only a human eye on the finished asset catches. Black beyond the cap
# is something else entirely (a blank frame, a window that moved mid-shot), fail the run rather
# than write the asset.
#
# The client's showcase mode inflates its window by the same metrics (MainWindow.Showcase.cs,
# ShowcaseFrameInset), so what survives this crop is the store size exactly: 2880x1800 at 200%.
# The geometry itself lives in screenshot-frame.ps1, where screenshot-frame.tests.ps1 can reach it
# without a window, a display, or a shutter.
# ---------------------------------------------------------------------------------------------
[CmdletBinding()]
param([string] $Out = (Join-Path $env:TEMP 'mailcal-windows.png'))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
. (Join-Path $PSScriptRoot 'screenshot-frame.ps1')

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ShotNative {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetSystemMetricsForDpi(int index, uint dpi);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

# The widest unpainted margin a window rect can carry, in physical px: the invisible resize border.
# ...ForDpi, not the plain call, so a mixed-DPI desktop doesn't answer for the primary display.
function Get-FrameInset([IntPtr] $h) {
  $SM_CXSIZEFRAME = 32; $SM_CYSIZEFRAME = 33; $SM_CXPADDEDBORDER = 92
  $dpi = [ShotNative]::GetDpiForWindow($h)
  if ($dpi -eq 0) { $dpi = 96 }
  $padded = [ShotNative]::GetSystemMetricsForDpi($SM_CXPADDEDBORDER, $dpi)
  return @{
    X = [ShotNative]::GetSystemMetricsForDpi($SM_CXSIZEFRAME, $dpi) + $padded
    Y = [ShotNative]::GetSystemMetricsForDpi($SM_CYSIZEFRAME, $dpi) + $padded
  }
}

$proc = Get-Process Mailcal -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) {
  Write-Error "no running Mailcal window found, launch it first (build-and-run.ps1)"
  exit 1
}
$h = $proc.MainWindowHandle
[ShotNative]::ShowWindow($h, 9) | Out-Null   # SW_RESTORE, in case it's minimized
[ShotNative]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 600                # let the frame compose before we grab it

$r = New-Object ShotNative+RECT
[ShotNative]::GetWindowRect($h, [ref] $r) | Out-Null
$w = $r.Right - $r.Left; $ht = $r.Bottom - $r.Top
if ($w -le 0 -or $ht -le 0) { Write-Error "the window has no visible bounds"; exit 1 }

$bmp = New-Object System.Drawing.Bitmap($w, $ht)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [ShotNative]::PrintWindow($h, $hdc, 2)  # 2 = PW_RENDERFULLCONTENT (WinUI needs it)
$g.ReleaseHdc($hdc); $g.Dispose()
if (-not $ok) {
  # PrintWindow can't always render a composed frame; fall back to copying the screen region.
  # That one comes off the composited desktop, so it has no unpainted margin to crop.
  $g2 = [System.Drawing.Graphics]::FromImage($bmp)
  $g2.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
  $g2.Dispose()
}

$inset = Get-FrameInset $h
$margin = Get-UnpaintedMargin $bmp $inset.X $inset.Y
$cropped = Remove-UnpaintedMargin $bmp $margin
if (-not [object]::ReferenceEquals($cropped, $bmp)) {
  $bmp.Dispose()
  $bmp = $cropped
  $w = $bmp.Width; $ht = $bmp.Height
}

$leftover = Get-BlackEdges $bmp
if ($leftover) {
  $bmp.Dispose()
  Write-Error @"
the capture still has a fully black $($leftover -join '/') edge after cropping, refusing to write $Out.
An unpainted margin wider than the window frame means the shot is wrong in some other way (a blank
frame, a window that moved mid-shot); the PNG would look valid to every size check and wrong to
every human.
  window rect: $($r.Left),$($r.Top) $($w)x$($ht)
  frame inset: $($inset.X)x$($inset.Y) px, cropped L=$($margin.left) R=$($margin.right) T=$($margin.top) B=$($margin.bottom)
"@
  exit 1
}

$dir = Split-Path -Parent $Out
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
# The dimensions note goes to stderr so stdout carries only the path, scripts/dev/screenshot.sh
# captures stdout and echoes it back.
[Console]::Error.WriteLine("==> screenshot: ${w}x${ht}")
Write-Output $Out
