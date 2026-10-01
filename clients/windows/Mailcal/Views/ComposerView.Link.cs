// The editor's requests to the composer (docs/composer-security.md, Gate 2), parsed by the core in
// EditorWebViewHost: the link dialog over the window, then its answer back into the message.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    private async void OnHostRequested(ComposerHostRequest request)
    {
        if (request is not ComposerHostRequest.Link link || XamlRoot is null)
        {
            return;
        }
        try
        {
            var answer = await LinkDialog.AskAsync(XamlRoot, link);
            // The WebView2 takes focus back as well as the page, or the restored selection sits in
            // the editor while keystrokes go to the shell.
            Editor.Focus(FocusState.Programmatic);
            await _editor.RunAsync(MailcalBindingsMethods.ComposerLinkAnswerScript(link.Id, answer));
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: the link dialog could not answer the editor ({ex.GetType().Name})");
        }
    }
}
