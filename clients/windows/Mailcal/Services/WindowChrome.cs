// The chrome every window of this app shares: the brand icon, the monitor scale a default size
// opens at, and the foreground grab that makes a window the user just asked for the one in front.
//
// One copy, because there are three windows now, not one: the shell, and the reading and composer
// windows a desktop opens beside it (docs/reading-window.md). Each of these has already been
// forgotten once by a window added later, and each is visible when it is: a window with no icon
// shows the system default in the title bar, the taskbar and Alt-Tab, next to siblings carrying the
// brand; a raw physical-pixel default opens as a sliver on a 200%-scale display; and a window this
// app raises from outside its own foreground, which is every OAuth redirect and every second
// launch, is ignored unless it asks past the OS's foreground lock.
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
    /// Two steps before the activation itself. A minimised window is restored, because a window the
    /// reader asked for is asked for on screen: that is the path a second double-click on a message
    /// whose window is minimised takes.
    /// <para>
    /// And after an out-of-process activation (the Microsoft OAuth redirect arrives through the
    /// browser) this process does not hold foreground rights at all, so a bare <c>Activate()</c> is
    /// ignored. Attaching briefly to the current foreground window's input queue is the standard way
    /// past the OS's foreground-stealing lock.
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
    /// Brings <paramref name="window"/> to the front and puts focus inside it once its content has
    /// loaded.
    /// </summary>
    /// <remarks>
    /// The front is taken once, and this does not watch it afterwards. Nothing of the app's own
    /// takes it back, because a window is shown only after the input that asked for it has been
    /// delivered (MainWindow.ReadingWindows.cs); a second grab here would be hedging against that
    /// and would be indistinguishable from a real regression in the log. The one thing that does
    /// take it back belongs to the shell, which knows which window it opened and when a browser is
    /// attaching under it.
    /// <para>
    /// Focus waits for <c>Loaded</c>, which is the first moment the tree has anything to focus.
    /// </para>
    /// <para>
    /// ⚠️ Nothing here may touch <c>FocusManager</c>'s static moves. They act on the element that
    /// holds focus NOW, which is in the mailbox, so a call meant to put focus in this window
    /// focused the mailbox instead and activated it: measured at 64 opens out of 64, a mailbox
    /// activation 7 to 22 ms after the window appeared, which is what put the window behind it.
    /// A window that needs focus somewhere specific asks that element, never the focus manager.
    /// </para>
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
            FocusInto(root);
            return;
        }
        void OnLoaded(object sender, RoutedEventArgs e)
        {
            root.Loaded -= OnLoaded;
            FocusInto(root);
        }
        root.Loaded += OnLoaded;
    }

    // Focus THIS window's first focusable element, named explicitly. The element-scoped calls only:
    // see Present's remarks for what the static move does instead.
    private static void FocusInto(FrameworkElement root)
    {
        try
        {
            if (FocusManager.FindFirstFocusableElement(root) is { } first)
            {
                _ = FocusManager.TryFocusAsync(first, FocusState.Programmatic);
            }
        }
        catch (Exception ex)
        {
            // Best-effort: the window is already in front, and the cost of failing here is the
            // reader pressing Tab once. Never worth taking the app down for.
            Log.Warn($"window: could not focus into it ({ex.GetType().Name})");
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
