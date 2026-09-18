// The shell's caption: handing the window over to the WinUI TitleBar control in MainWindow.xaml,
// and what that control forwards back. Split out of MainWindow.xaml.cs so the shell file stays about
// the shell.
//
// Why the app draws its own caption at all, and what the framework still will not do for it, are
// Services/WindowCaption.cs's: the reading and composer windows draw one too, on the same terms.
// What is here is the shell's own, the pane toggle and the search field it carries beside the name.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    // The ROW, not the TitleBar control alone: the search field is a sibling of it rather than its
    // content (MainWindow.xaml says why), and only what is inside the element handed over here is
    // excluded from the drag region. Given the control alone, the field would sit on bare caption
    // and a click on it would drag the window instead.
    private void InitTitleBar() => WindowCaption.Extend(this, CaptionRow);

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
    /// Whether the mail surface's own chrome belongs on screen: the shell is up, and mail is what
    /// it is showing. An <c>x:Bind</c> function rather than a model property so the two answers
    /// cannot drift apart, and an *instance* method for the reason <see cref="IsVisible"/> is one.
    /// It is what both the caption's search field and the actions bar under it are bound to.
    /// </summary>
    public Visibility MailChromeShown(Visibility shell, Visibility mail) =>
        shell == Visibility.Visible && mail == Visibility.Visible
            ? Visibility.Visible
            : Visibility.Collapsed;

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
    private void OnTitleBarPaneToggle(TitleBar sender, object args)
    {
        Nav.IsPaneOpen = !Nav.IsPaneOpen;
        if (Nav.IsPaneOpen)
        {
            // The framework shut every tree on the way down and does not put them back, and the
            // shell deliberately did not tell the core (OnExpandedChanged says why). So the trees
            // the user left open are the core's to restore, and this is the moment.
            SyncNavItems();
        }
    }
}
