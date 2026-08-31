// The window's light/dark appearance: the core's persisted choice, applied to the content root.
//
// Applied on the ELEMENT rather than through Application.RequestedTheme, which can only be set once
// before any content exists. Pinning it there would make "Use system setting" unreachable without a
// restart: an element back on ElementTheme.Default inherits the application's theme, which would by
// then be the override rather than the desktop's.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal;

public sealed partial class MainWindow
{
    /// <summary>
    /// Paints the window in <paramref name="appearance"/>. The caption buttons follow through
    /// <c>ActualThemeChanged</c> (MainWindow.TitleBar.cs), and the calendar surface rebuilds its
    /// palette on the same event.
    /// </summary>
    internal void ApplyAppearance(Appearance appearance)
    {
        if (Content is FrameworkElement root)
        {
            root.RequestedTheme = Theme(appearance);
        }
    }

    /// <summary>
    /// The element theme for <paramref name="appearance"/>, <c>Default</c> for "follow the host",
    /// which is what makes a live desktop light/dark switch still reach the app.
    /// </summary>
    internal static ElementTheme Theme(Appearance appearance) => appearance switch
    {
        Appearance.Light => ElementTheme.Light,
        Appearance.Dark => ElementTheme.Dark,
        _ => ElementTheme.Default,
    };
}
