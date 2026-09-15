// The chrome every window of this app shares: the brand icon, the monitor scale a default size
// opens at, and the foreground grab that makes a window the user just asked for the one in front.
//
// One copy, because there are three windows now, not one: the shell, and the reading and composer
// windows a desktop opens beside it (docs/reading-window.md). Each of these has already been
// forgotten once by a window added later, and each is visible when it is: a window with no icon
// shows the system default in the title bar, the taskbar and Alt-Tab, next to siblings carrying the
// brand; a raw physical-pixel default opens as a sliver on a 200%-scale display; and a window that
// never takes the foreground is drawn in front for a moment and then disappears behind the window
// it was opened from.
//
// Dress() exists so a new window cannot half-do it: the icon is not a line to remember beside the
// title and the size, it comes with them.

using System;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.UI.Windowing;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.Graphics;

namespace Allodia.Mailcal.Services;

/// <summary>What every window this app opens has in common.</summary>
internal static class WindowChrome
{
    /// <summary>
    /// Gives <paramref name="window"/> its content, its title, its opening size and the brand icon.
    /// </summary>
    /// <remarks>
    /// The content is wrapped in a <see cref="Grid"/> because that root is what carries the
    /// appearance: <c>RequestedTheme</c> is set on an element, never on the application, so a
    /// window needs one element of its own to paint.
    /// </remarks>
    internal static void Dress(Window window, UIElement content, string title, SizeInt32 logical)
    {
        var root = new Grid();
        root.Children.Add(content);
        window.Content = root;
        // Window.Title is what the taskbar, Alt-Tab and UI Automation read.
        window.Title = title;
        window.AppWindow.Resize(ToDpi(window, logical));
        SetAppIcon(window.AppWindow);
    }

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
    /// Makes <paramref name="window"/> the window in front, restoring it first if it is minimised.
    /// </summary>
    /// <remarks>
    /// <c>Activate()</c> alone is not enough, and the two cases it fails in are different.
    /// <para>
    /// After an out-of-process activation (the Microsoft OAuth redirect arrives through the
    /// browser) this process does not hold foreground rights at all, so a bare Activate is ignored.
    /// Attaching briefly to the current foreground window's input queue is the standard way past
    /// the OS's foreground-stealing lock.
    /// </para>
    /// <para>
    /// The second case is a window this app opens while one of its own windows is mid-input, which
    /// is every reading window: a double-click is still being delivered to the mailbox when the new
    /// window appears. Activate shows it and puts it at the top of the z-order but leaves the
    /// foreground where it was, and Windows then restores its foreground window to the top, so the
    /// new window is drawn in front for about fifty milliseconds and then vanishes behind the
    /// mailbox. Taking the foreground is what makes it stay.
    /// </para>
    /// </remarks>
    internal static void BringToForeground(Window window)
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(window);
        if (IsIconic(hwnd))
        {
            ShowWindow(hwnd, SwRestore);
        }
        var foreground = GetForegroundWindow();
        if (foreground != hwnd)
        {
            var foreThread = GetWindowThreadProcessId(foreground, out _);
            var thisThread = GetCurrentThreadId();
            var attached = foreThread != thisThread && AttachThreadInput(thisThread, foreThread, true);
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);
            if (attached)
            {
                AttachThreadInput(thisThread, foreThread, false);
            }
        }
        window.Activate();
    }

    /// <summary>
    /// Brings <paramref name="window"/> to the front and moves keyboard focus into it.
    /// </summary>
    /// <remarks>
    /// For a window this app opens out of another one. Taking the foreground is not enough on its
    /// own: the mailbox's message list still holds keyboard focus, and the next snapshot reconciles
    /// the rows under it, which restores focus to the list and activates the mailbox with it. That
    /// is what dropped a reading window behind the mailbox about fifty milliseconds after it
    /// appeared, and it is why the focus moves as well as the foreground.
    /// </remarks>
    internal static void Present(Window window)
    {
        BringToForeground(window);
        if (window.Content is not FrameworkElement root)
        {
            return;
        }
        if (root.IsLoaded)
        {
            MoveFocusInto(root);
            return;
        }
        // A window presented the moment it is built has no loaded visual tree yet, and asking the
        // focus manager to search one throws. Waiting for Loaded is not a nicety: the throw came
        // out of a double-click handler and took the process with it.
        //
        // The foreground is taken AGAIN there, not only the focus. The mailbox's snapshot lands
        // inside the few tens of milliseconds this window spends loading, and whichever of the two
        // arrives second wins; asking once more once this window is real closes that gap.
        void OnLoaded(object sender, RoutedEventArgs e)
        {
            root.Loaded -= OnLoaded;
            BringToForeground(window);
            MoveFocusInto(root);
        }
        root.Loaded += OnLoaded;
    }

    private static void MoveFocusInto(FrameworkElement root)
    {
        try
        {
            FocusManager.TryMoveFocus(
                FocusNavigationDirection.Next, new FindNextElementOptions { SearchRoot = root });
        }
        catch (Exception ex)
        {
            // Best-effort: the window is already in front, and the only cost of failing here is
            // the mailbox winning the focus back. Never worth taking the app down for.
            Log.Warn($"window: could not move focus into it ({ex.GetType().Name})");
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

    private const int SwRestore = 9;

    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern bool ShowWindow(IntPtr hwnd, int cmdShow);
    [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern bool BringWindowToTop(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll")] private static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool attach);
    [DllImport("kernel32.dll")] private static extern uint GetCurrentThreadId();
}
