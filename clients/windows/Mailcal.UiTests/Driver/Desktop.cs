// What a window is to the desktop rather than to the app: which one is in front, whether it wears
// an icon, and the caption buttons the system draws beside the app's own caption. None of it is a
// XAML element, so the agent cannot see it.

using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Automation;
using System.Xml.Linq;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class Desktop
{
    /// <summary>The title of whichever window holds the foreground.</summary>
    internal static string ForegroundTitle()
    {
        var text = new StringBuilder(512);
        GetWindowTextW(GetForegroundWindow(), text, text.Capacity);
        return text.ToString();
    }

    /// <summary>
    /// Whether <paramref name="hwnd"/> carries an icon of its own, small and big: WM_GETICON is what
    /// the title bar, the taskbar and Alt-Tab read. WinUI 3 puts none on a window by itself, so a
    /// window that forgets to ask for one wears the system's default beside its siblings.
    /// </summary>
    internal static bool HasIcon(long hwnd) =>
        SendMessageW((nint)hwnd, WmGetIcon, 0, 0) != 0 && SendMessageW((nint)hwnd, WmGetIcon, 1, 0) != 0;

    /// <summary>
    /// The window's minimise, maximise and close buttons, keyed by AutomationId. The system draws
    /// them in a pane of the window's own, outside the XAML tree, so they are read through UI
    /// Automation, scoped to that pane: a search of the whole window would cross a message body's
    /// browser.
    /// </summary>
    internal static IReadOnlyDictionary<string, System.Windows.Rect> CaptionButtons(long hwnd)
    {
        var window = AutomationElement.FromHandle((nint)hwnd);
        var pane = window.FindFirst(TreeScope.Children,
                new PropertyCondition(AutomationElement.ClassNameProperty, "ReunionWindowingCaptionControls"))
            ?? throw new InvalidOperationException($"window {hwnd} has no system caption pane, so its caption buttons cannot be read");
        var buttons = new Dictionary<string, System.Windows.Rect>();
        foreach (var id in new[] { "Minimize", "Maximize", "Close" })
        {
            var button = pane.FindFirst(TreeScope.Descendants, new PropertyCondition(AutomationElement.AutomationIdProperty, id))
                ?? throw new InvalidOperationException(
                    $"the window carries no '{id}' caption button. Extending into the caption hides the system strip and leaves these three in place; a window that lost them cannot be closed with a pointer at all");
            buttons[id] = button.Current.BoundingRectangle;
        }
        return buttons;
    }

    /// <summary>How tall a caption is in this app, in the units XAML is written in, read from
    /// App.xaml where every window's caption reads it: a copy here would let the two disagree.</summary>
    internal static double CaptionHeightDip()
    {
        var path = Path.Combine(App.Repo, "clients", "windows", "Mailcal", "App.xaml");
        var value = XDocument.Load(path).Descendants()
            .FirstOrDefault(e => e.Name.LocalName == "Double" && e.Attributes().Any(a => a.Name.LocalName == "Key" && a.Value == "AppCaptionHeight"))
            ?? throw new InvalidOperationException($"{path} no longer defines AppCaptionHeight");
        return double.Parse(value.Value, System.Globalization.CultureInfo.InvariantCulture);
    }

    private const uint WmGetIcon = 0x007F;

    [DllImport("user32.dll")] private static extern nint GetForegroundWindow();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowTextW(nint hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern nint SendMessageW(nint hwnd, uint message, nint wParam, nint lParam);
}
