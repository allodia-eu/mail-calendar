// The window's caption: handing it over to the WinUI TitleBar control in MainWindow.xaml, and the
// one part of it the framework will not theme for us. Split out of MainWindow.xaml.cs so the shell
// file stays about the shell.
//
// Why a custom caption at all: the system one is a separate surface from the app's content, so it
// does not read the app's theme, on a dark-mode desktop the app came up dark with a pale strip
// across the top of it. The TitleBar control *is* content, so it inherits ActualTheme like every
// other control, and it owns the drag regions, the caption-button spacing (including RTL) and the
// min-drag region, none of which then has to be hand-computed.
// https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/title-bar

using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // Hides the system caption and nominates the XAML TitleBar as the draggable region. Order
    // matters: SetTitleBar on a window that has not extended its content is a no-op.
    private void InitTitleBar()
    {
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        // The minimise / maximise / close buttons are drawn by the SYSTEM, on a surface that is not
        // in the XAML tree, so unlike the rest of the caption they do not inherit ActualTheme, and
        // left alone they keep whatever mode the window was created under. Flip the desktop to light
        // while the app is running (which Windows itself does on a sunrise schedule) and the glyphs
        // stay white on a now-pale bar: the close button effectively disappears. PreferredTheme is
        // the property that re-themes them, so mirror the content's own theme onto it, now, and on
        // every change for as long as the window lives.
        var root = (FrameworkElement)Content;
        root.ActualThemeChanged += (sender, _) => ApplyCaptionButtonTheme(sender.ActualTheme);
        ApplyCaptionButtonTheme(root.ActualTheme);
    }

    // ActualTheme resolves Default, so it is only ever Light or Dark here.
    private void ApplyCaptionButtonTheme(ElementTheme theme) =>
        AppWindow.TitleBar.PreferredTheme =
            theme == ElementTheme.Dark ? TitleBarTheme.Dark : TitleBarTheme.Light;

    /// <summary>
    /// Whether a <see cref="Visibility"/> is <c>Visible</c>, an <c>x:Bind</c> function helper, so
    /// the title bar's pane toggle can follow the shell's own visibility without the model growing a
    /// second, parallel boolean that could drift from it. Deliberately an *instance* method: x:Bind
    /// resolves a bare function name against the page instance, and a static one fails to compile
    /// with CS0176.
    /// </summary>
#pragma warning disable CA1822 // see above, x:Bind cannot call this if it is static
    public bool IsVisible(Visibility visibility) => visibility == Visibility.Visible;

    /// <summary>
    /// A <see cref="Visibility"/> from a flag, for the banner strip, whose bars bind both
    /// <c>IsOpen</c> and <c>Visibility</c> to the same one (MainWindow.xaml says why). A function
    /// rather than the <c>BoolToVisibility</c> converter because an <c>x:Bind</c> converter needs a
    /// FrameworkElement to resolve against and this file's root is a <c>Window</c>. An instance
    /// method for the same reason as <see cref="IsVisible"/>.
    /// </summary>
    public Visibility Shown(bool flag) => flag ? Visibility.Visible : Visibility.Collapsed;
#pragma warning restore CA1822

    // The pane toggle lives in the title bar (the NavigationView's own is hidden), which is the
    // Fluent guidance when a custom title bar exists, so the collapse it used to do itself is
    // forwarded here.
    private void OnTitleBarPaneToggle(TitleBar sender, object args) => Nav.IsPaneOpen = !Nav.IsPaneOpen;
}
