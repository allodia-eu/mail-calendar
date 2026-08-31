// Window placement: the brand icon, reopening where the user left the window (position, size,
// maximised), the DPI scaling that makes the first-run default the intended size on any display,
// and the foreground grab an out-of-process OAuth redirect needs. Split out of MainWindow.xaml.cs
// to keep that file under the 500-line limit.

using System.Runtime.InteropServices;
using Allodia.Mailcal.Services;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Windows.Graphics;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // The size the window opens at on first run (before the user has sized it). Logical units,
    // scaled by the monitor DPI in RestoreWindowState so it looks the same on a 200%-scale
    // display as on a 100% one, a physical-pixel default rendered as a tiny window on HiDPI.
    private static readonly SizeInt32 DefaultLogicalSize = new(1180, 760);

    // The latest restored (un-maximised) bounds and maximised flag, tracked as the window moves
    // so OnClosed can persist whatever it last was, see RestoreWindowState / OnAppWindowChanged.
    private RectInt32 _restoredBounds;
    private bool _maximized;

    // Puts the brand icon in the title bar (and this window's taskbar button). WinUI 3 does NOT
    // surface the exe's embedded <ApplicationIcon> in the title bar on its own, so we point the
    // AppWindow at the same app.ico, laid down next to the exe / under Images\ in the MSIX by the
    // csproj Content item. Best-effort: a missing file just leaves the system default (the icon is
    // cosmetic, never worth failing window creation over). AppContext.BaseDirectory resolves to the
    // exe dir when unpackaged and the package install root when packaged, so the one path fits both.
    private void SetWindowIcon()
    {
        var iconPath = Path.Combine(AppContext.BaseDirectory, "Images", "app.ico");
        if (File.Exists(iconPath))
        {
            AppWindow.SetIcon(iconPath);
        }
    }

    // Reopens the window where the user last left it, same position, size, and maximised state,
    // falling back to a DPI-scaled default on first run or when the saved spot is off-screen (e.g.
    // a monitor was unplugged). Pairs with OnAppWindowChanged / OnClosed, which save the placement.
    private void RestoreWindowState()
    {
        // A screenshot run pins its own store-valid frame instead (ShowcaseInit -> ShowcaseSizeWindow),
        // so the developer's saved placement neither sizes the capture nor gets overwritten by it.
        if (ShowcaseMode.IsOn)
        {
            return;
        }
        var presenter = AppWindow.Presenter as OverlappedPresenter;
        if (WindowStateStore.Read() is { } saved)
        {
            var bounds = new RectInt32(saved.X, saved.Y, saved.Width, saved.Height);
            // Honour the saved position only if a display still contains it; otherwise keep the
            // size but let the OS place the window so it can't open onto a now-absent monitor.
            if (DisplayArea.GetFromRect(bounds, DisplayAreaFallback.None) is not null)
            {
                AppWindow.MoveAndResize(bounds);
            }
            else
            {
                AppWindow.Resize(new SizeInt32(saved.Width, saved.Height));
                bounds = new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, saved.Width, saved.Height);
            }
            _restoredBounds = bounds;
            _maximized = saved.Maximized;
            if (saved.Maximized)
            {
                presenter?.Maximize();
            }
        }
        else
        {
            var size = ScaleToDpi(DefaultLogicalSize);
            AppWindow.Resize(size);
            _restoredBounds = new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, size.Width, size.Height);
            _maximized = false;
        }
    }

    // Tracks the live placement so OnClosed can persist whatever the window last was. Bounds are
    // captured only while Restored, so the saved rectangle is always the un-maximised size, even
    // when the user closes maximised, un-maximising later returns to it. A minimised window has a
    // bogus off-screen position, so that state is ignored.
    private void OnAppWindowChanged(AppWindow sender, AppWindowChangedEventArgs args)
    {
        if (sender.Presenter is not OverlappedPresenter presenter)
        {
            return;
        }
        switch (presenter.State)
        {
            case OverlappedPresenterState.Restored:
                _restoredBounds = new RectInt32(
                    sender.Position.X, sender.Position.Y, sender.Size.Width, sender.Size.Height);
                _maximized = false;
                break;
            case OverlappedPresenterState.Maximized:
                _maximized = true;
                break;
        }
    }

    private void OnClosed(object sender, WindowEventArgs args)
    {
        Log.Info("window closed");
        // A capture pass relaunches the app once per screen per language, each time at the pinned
        // showcase frame. Persisting that would quietly replace the developer's own window placement
        // This is the same trap the Android capture path avoids by restoring the per-app locale it sets.
        if (ShowcaseMode.IsOn)
        {
            return;
        }
        WindowStateStore.Write(new WindowPlacement(
            _restoredBounds.X, _restoredBounds.Y, _restoredBounds.Width, _restoredBounds.Height, _maximized));
    }

    // Multiplies a logical size by the window's monitor scale (DPI / 96) so the first-run default
    // is the intended size on any display, a raw physical-pixel default opened as a sliver on the
    // HiDPI screens this app's dev box uses.
    private SizeInt32 ScaleToDpi(SizeInt32 logical)
    {
        var dpi = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this));
        if (dpi == 0)
        {
            return logical; // GetDpiForWindow shouldn't fail, but never scale by zero.
        }
        var scale = dpi / 96.0;
        return new SizeInt32((int)(logical.Width * scale), (int)(logical.Height * scale));
    }

    // Force the window to the foreground after an out-of-process activation (the Microsoft OAuth
    // redirect arrives through the browser, so this process doesn't hold foreground rights and a
    // bare Activate() is ignored, the more so as we're the redirected-to primary instance, not
    // the one the shell launched). Restore if minimised, then take foreground: when another app
    // owns it, briefly attach to its input-thread queue, the standard way to bypass the OS's
    // foreground-stealing lock. Called on the UI thread from Program's activation handler.
    public void BringToForeground()
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        if (IsIconic(hwnd))
        {
            ShowWindow(hwnd, SW_RESTORE);
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
        Activate();
    }

    private const int SW_RESTORE = 9;

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
