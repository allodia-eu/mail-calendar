// The "your name" step: the one question asked once an account connects (docs/sending.md).
//
// After the add rather than before it. The screen that adds an account is the address field and
// nothing else (docs/onboarding.md), and only a connected account can be asked what its provider
// already calls this person, which on a Microsoft or Google account turns the step into a
// confirmation.

using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The step that asks what to call the sender of a newly connected account's mail.</summary>
internal static class SenderNameDialog
{
    /// <summary>
    /// Asks for the name, seeded with <paramref name="suggestion"/>, and returns what to store;
    /// <c>null</c> when the person skipped.
    /// </summary>
    /// <remarks>
    /// An empty return is not a skip: it is "send as my address alone", which is a real answer
    /// and clears any name already stored. Only <c>null</c> means nothing was decided.
    /// </remarks>
    public static async Task<string?> AskAsync(XamlRoot root, string suggestion)
    {
        var box = new TextBox
        {
            Text = suggestion,
            PlaceholderText = L10n.SetupSenderNameField(),
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, L10n.SetupSenderNameField());
        var panel = new StackPanel { Spacing = 12 };
        panel.Children.Add(new TextBlock
        {
            Text = L10n.SetupSenderNameDescription(),
            TextWrapping = TextWrapping.Wrap,
        });
        panel.Children.Add(box);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = L10n.SetupSenderNameTitle(),
            Content = panel,
            PrimaryButtonText = L10n.SetupSenderNameContinue(),
            CloseButtonText = L10n.SetupSenderNameSkip(),
            // Return commits, which is what a field in a dialog is expected to do. Skipping is a
            // deliberate press, not the path of least resistance.
            DefaultButton = ContentDialogButton.Primary,
        };
        var result = await DialogHelper.ShowAsync(dialog);
        return result == ContentDialogResult.Primary ? box.Text : null;
    }
}
