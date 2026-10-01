// The link dialog: the words a link is shown as and where it points, the way Outlook asks. The
// address is completed and checked by the core as it is typed (ComposerLinkAddress), the same rule
// the answer is held to, so Apply is enabled exactly when the link can be made.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

internal static class LinkDialog
{
    /// <summary>
    /// Asks for the link <paramref name="request"/> starts from, and answers with how the dialog
    /// closed. A dialog that could not open (another is already up) answers Cancel.
    /// </summary>
    internal static async Task<ComposerLinkAnswer> AskAsync(XamlRoot root, ComposerHostRequest.Link request)
    {
        var text = new TextBox { Header = L10n.EditorLinkText(), Text = request.Text };
        var urlScope = new InputScope();
        urlScope.Names.Add(new InputScopeName(InputScopeNameValue.Url));
        var address = new TextBox
        {
            Header = L10n.EditorLinkAddress(),
            Text = request.Address,
            InputScope = urlScope,
            IsSpellCheckEnabled = false,
        };
        var hint = new InfoBar
        {
            Message = L10n.EditorLinkInvalid(),
            Severity = InfoBarSeverity.Error,
            IsClosable = false,
            IsOpen = false,
        };
        var panel = new StackPanel { Spacing = 12, MinWidth = 360 };
        panel.Children.Add(text);
        panel.Children.Add(address);
        panel.Children.Add(hint);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = request.Address.Length == 0 ? L10n.EditorLinkDialogInsert() : L10n.EditorLinkDialogEdit(),
            Content = panel,
            PrimaryButtonText = L10n.EditorLinkApply(),
            // An empty label hides the button, so Remove is offered only on a link.
            SecondaryButtonText = request.Removable ? L10n.EditorLinkRemove() : string.Empty,
            CloseButtonText = L10n.ActionCancel(),
            DefaultButton = ContentDialogButton.Primary,
        };

        void Refresh()
        {
            var typed = address.Text;
            var valid = MailcalBindingsMethods.ComposerLinkAddress(typed) is not null;
            dialog.IsPrimaryButtonEnabled = valid;
            // Said only once something has been typed: an empty field is unfinished, not wrong.
            hint.IsOpen = !string.IsNullOrWhiteSpace(typed) && !valid;
        }
        Refresh();
        address.TextChanged += (_, _) => Refresh();
        dialog.Opened += (_, _) => address.Focus(FocusState.Programmatic);

        return await DialogHelper.ShowAsync(dialog) switch
        {
            ContentDialogResult.Primary => new ComposerLinkAnswer.Apply(text.Text, address.Text),
            ContentDialogResult.Secondary => new ComposerLinkAnswer.Remove(),
            _ => new ComposerLinkAnswer.Cancel(),
        };
    }
}
