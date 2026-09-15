// The chrome every window of this app shares: the brand icon, and the monitor scale a default size
// opens at.
//
// One copy, because there are three windows now, not one: the shell, and the reading and composer
// windows a desktop opens beside it (docs/reading-window.md). Both failures are visible and both
// are easy to leave out of a window added later. A window with no icon shows the system default in
// the title bar, the taskbar and Alt-Tab, next to siblings that carry the brand; and a raw
// physical-pixel default size opens as a sliver on a 200%-scale display.

using System;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Windows.Graphics;

namespace Allodia.Mailcal.Services;

/// <summary>What every window this app opens has in common.</summary>
internal static class WindowChrome
{
    /// <summary>
    /// Puts the brand icon in <paramref name="window"/>'s title bar and taskbar button.
    /// </summary>
    /// <remarks>
    /// WinUI 3 does not surface the exe's embedded <c>ApplicationIcon</c> on a window by itself, so
    /// every window points its AppWindow at the same app.ico, laid down next to the exe (and under
    /// Images\ in the MSIX) by the csproj Content item. Best-effort: a missing file leaves the
    /// system default, which is cosmetic and never worth failing window creation over.
    /// AppContext.BaseDirectory is the exe directory unpackaged and the package install root
    /// packaged, so the one path fits both.
    /// </remarks>
    internal static void SetAppIcon(AppWindow window)
    {
        var iconPath = Path.Combine(AppContext.BaseDirectory, "Images", "app.ico");
        if (File.Exists(iconPath))
        {
            window.SetIcon(iconPath);
        }
    }

    /// <summary>
    /// <paramref name="logical"/> multiplied by <paramref name="window"/>'s monitor scale
    /// (DPI / 96), so a default size is the intended size on any display.
    /// </summary>
    internal static SizeInt32 ToDpi(Window window, SizeInt32 logical)
    {
        // DpiOf never answers zero, so this can never scale by it.
        var scale = DpiOf(window) / 96.0;
        return new SizeInt32((int)(logical.Width * scale), (int)(logical.Height * scale));
    }

    /// <summary>
    /// The scale of the display <paramref name="window"/> is on, in dots per inch, or 96 (100%)
    /// when the system declines to say.
    /// </summary>
    /// <remarks>
    /// Per window rather than for the primary display, which is the only form that answers
    /// correctly on a mixed-DPI desktop.
    /// </remarks>
    internal static uint DpiOf(Window window)
    {
        var dpi = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(window));
        return dpi == 0 ? 96u : dpi;
    }

    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr hwnd);
}
