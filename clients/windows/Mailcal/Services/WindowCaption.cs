// The strip across the top of a window: who draws it, how tall it is, and what the framework still
// will not theme for us. Three windows have one, the shell and the reading and composer windows
// beside it (docs/reading-window.md), and everything they share about it is here.
//
// Why a custom caption at all: the system's is a separate surface from the app's content, so it does
// not read the app's theme, and on a dark-mode desktop the app came up dark with a pale strip across
// the top of it. The TitleBar control *is* content, so it inherits ActualTheme like every other
// control, and it owns the drag regions, the caption-button spacing (including RTL) and the min-drag
// region, none of which then has to be hand-computed.
// https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/title-bar
//
// Each window writes its own caption, because they differ: the shell's carries a pane toggle and
// the search field (MainWindow.xaml), the other two carry the brand icon and the window's name and
// nothing else (Views/WindowShell.xaml). What they have in common is Extend(), so a window cannot
// take its caption over and miss half of what that costs.

using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;

namespace Allodia.Mailcal.Services;

/// <summary>The caption this app draws in place of the system's.</summary>
internal static class WindowCaption
{
    /// <summary>
    /// Hands <paramref name="window"/>'s caption over to <paramref name="dragRegion"/>, and keeps
    /// the system-drawn caption buttons in the content's own theme and at the content's own height.
    /// </summary>
    /// <remarks>
    /// Order matters: <c>SetTitleBar</c> on a window that has not extended its content is a no-op.
    /// <para>
    /// <paramref name="dragRegion"/> is what the drag region is computed from, minus the interactive
    /// things inside it, so it is the whole caption row and not only the TitleBar control when the
    /// two differ: only what is inside the element handed over here is excluded from the drag.
    /// </para>
    /// </remarks>
    internal static void Extend(Window window, UIElement dragRegion)
    {
        window.ExtendsContentIntoTitleBar = true;
        window.SetTitleBar(dragRegion);

        var titleBar = window.AppWindow.TitleBar;
        // The minimise / maximise / close buttons are drawn by the SYSTEM, on a surface that is not
        // in the XAML tree, so neither their height nor their theme follows the caption they sit in.
        //
        // Height first. They are 32 epx unless asked otherwise, and a caption here is 48
        // (AppCaptionHeight, App.xaml), so left alone they are top-aligned against a row half again
        // as tall: their glyphs sit a row's worth above the icon and the app's name beside them, and
        // the target the pointer (or a finger) has to find is a third shorter than the bar it is in.
        titleBar.PreferredHeightOption = TitleBarHeightOption.Tall;

        // Theme second, and it has to be mirrored rather than set once: left alone the buttons keep
        // whatever mode the window was created under. Flip the desktop to light while the app is
        // running (which Windows itself does on a sunrise schedule) and the glyphs stay white on a
        // now-pale bar: the close button effectively disappears.
        if (window.Content is FrameworkElement root)
        {
            root.ActualThemeChanged += (sender, _) => ApplyButtonTheme(titleBar, sender.ActualTheme);
            ApplyButtonTheme(titleBar, root.ActualTheme);
        }
    }

    // ActualTheme resolves Default, so it is only ever Light or Dark here.
    private static void ApplyButtonTheme(AppWindowTitleBar titleBar, ElementTheme theme) =>
        titleBar.PreferredTheme = theme == ElementTheme.Dark ? TitleBarTheme.Dark : TitleBarTheme.Light;
}
