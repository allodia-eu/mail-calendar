// Serializes ContentDialogs and provides a confirm shortcut. WinUI throws
// InvalidOperationException ("Only a single ContentDialog can be open at any time") if a
// second dialog opens while one is showing, which a fast double-click on a Compose/Reply/
// destructive button would otherwise trigger inside an async void handler (an unhandled
// crash). Routing every dialog through here drops the second show instead.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

/// <summary>Shows ContentDialogs one at a time and builds confirm dialogs.</summary>
internal static class DialogHelper
{
    private static bool _open;

    /// <summary>Whether a dialog is on screen, so a second show would be dropped.</summary>
    /// <remarks>
    /// A dropped show returns <c>None</c>, which is the same answer the close button gives, so a
    /// caller whose question may be asked only once cannot tell the two apart afterwards. It asks
    /// this first instead, and comes back later.
    /// </remarks>
    public static bool IsShowing => _open;

    /// <summary>Shows <paramref name="dialog"/>, or returns <c>None</c> if one is already open.</summary>
    public static async Task<ContentDialogResult> ShowAsync(ContentDialog dialog)
    {
        if (_open)
        {
            return ContentDialogResult.None;
        }
        _open = true;
        FrameworkElement? themeRoot = null;
        void SyncTheme(FrameworkElement sender, object _) => dialog.RequestedTheme = sender.ActualTheme;
        try
        {
            // A ContentDialog opens in the XamlRoot's popup tree, not under the element that
            // created it. Mirror the content root while the popup is open, including an appearance
            // pick in this dialog and a host-theme change while following the system.
            if (dialog.RequestedTheme == ElementTheme.Default
                && dialog.XamlRoot?.Content is FrameworkElement root)
            {
                themeRoot = root;
                root.ActualThemeChanged += SyncTheme;
                dialog.RequestedTheme = root.ActualTheme;
            }
            return await dialog.ShowAsync();
        }
        finally
        {
            if (themeRoot is not null)
            {
                themeRoot.ActualThemeChanged -= SyncTheme;
            }
            _open = false;
        }
    }

    /// <summary>Shows a standard confirm dialog (destructive primary + a close button). The close
    /// button is "Cancel" unless <paramref name="closeText"/> names the specific way out, the
    /// discard-draft prompt, for one, offers "Keep editing", which says what staying actually
    /// does.</summary>
    public static Task<ContentDialogResult> ConfirmAsync(
        XamlRoot root, string title, string content, string primaryText, string? closeText = null) =>
        ShowAsync(new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = content,
            PrimaryButtonText = primaryText,
            CloseButtonText = closeText ?? L10n.ActionCancel(),
            DefaultButton = ContentDialogButton.Close,
        });

    /// <summary>Asks which occurrences a write on a repeating event meant: <c>Primary</c> is this
    /// event alone, <c>Secondary</c> is the whole series, and closing writes nothing. The core has
    /// no default here and neither does this, acting on one Tuesday and acting on the standup are
    /// different requests, and only the user knows which they meant.</summary>
    public static Task<ContentDialogResult> ScopeAsync(XamlRoot root, string title) =>
        ShowAsync(new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            PrimaryButtonText = L10n.EventSeriesScopeThis(),
            SecondaryButtonText = L10n.EventSeriesScopeAll(),
            CloseButtonText = L10n.ActionCancel(),
            DefaultButton = ContentDialogButton.Close,
        });
}
