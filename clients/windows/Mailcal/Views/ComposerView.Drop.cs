// Files dragged onto the composer, and the question a picture raises.
//
// A drop is handled NATIVELY, not by the page. The editor bundle refuses `drop`, because web code
// only ever sees a `File` with no path: it could neither hand the bytes to Rust for a streamed send
// nor put a removable row in the attachment list. The host resolves the drop to a real path, so
// both work, and the page is handed a picture only when the user asks for one.
//
// A picture raises the question the other formats do not: it can be shown where the user is typing
// (an inline `cid:` part, what Outlook does) or sent as a file to download. Everything else is
// simply attached. The question is asked once for the whole drop.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using System.Text.Json;
using uniffi.mailcal_bindings;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    private void OnDragOver(object sender, DragEventArgs e)
    {
        if (e.DataView.Contains(StandardDataFormats.StorageItems))
        {
            e.AcceptedOperation = DataPackageOperation.Copy;
            e.Handled = true;
        }
    }

    private async void OnDrop(object sender, DragEventArgs e)
    {
        if (!e.DataView.Contains(StandardDataFormats.StorageItems))
        {
            return;
        }
        // Taken before the first await: the deferral keeps the data package alive across it.
        var deferral = e.GetDeferral();
        try
        {
            var items = await e.DataView.GetStorageItemsAsync();
            var files = items.OfType<StorageFile>()
                .Where(file => !string.IsNullOrEmpty(file.Path))
                .ToList();
            if (files.Count == 0)
            {
                return;
            }
            var pictures = files.Where(IsPicture).ToList();
            Attach(files.Where(file => !IsPicture(file)));
            if (pictures.Count > 0)
            {
                await AskAboutPicturesAsync(pictures);
            }
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't take a dropped file ({ex.GetType().Name})");
            ShowError(L10n.ComposeImageFailed());
        }
        finally
        {
            deferral.Complete();
        }
    }

    // Whether a dropped file is worth asking about. Windows' own guess from the name, which is
    // enough to choose a question; the core sniffs the bytes before anything is shown
    // (ComposerImageDataUrl), so a mislabelled file still cannot become an `<img>`.
    private static bool IsPicture(StorageFile file) =>
        file.ContentType.StartsWith("image/", StringComparison.OrdinalIgnoreCase);

    private void Attach(IEnumerable<StorageFile> files)
    {
        var added = false;
        foreach (var file in files)
        {
            _attachments.Add(new PickedComposerAttachment(
                new ComposerFileAttachment(file.Path, file.Name, MediaType(file))));
            added = true;
        }
        if (added)
        {
            AttachmentList.ItemsSource = null;
            AttachmentList.ItemsSource = _attachments;
        }
    }

    private async Task AskAboutPicturesAsync(IReadOnlyList<StorageFile> pictures)
    {
        // Three answers, so a ContentDialog rather than the confirm helper: Primary shows it in
        // the message, Secondary sends it as a file, and closing leaves the message alone.
        var answer = await DialogHelper.ShowAsync(new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = L10n.ComposeImageDropTitle(),
            Content = L10n.ComposeImageDropBody(),
            PrimaryButtonText = L10n.ComposeImageDropInline(),
            SecondaryButtonText = L10n.ComposeImageDropAttach(),
            CloseButtonText = L10n.ActionCancel(),
            DefaultButton = ContentDialogButton.Primary,
        });
        if (answer == ContentDialogResult.Secondary)
        {
            Attach(pictures);
            return;
        }
        if (answer != ContentDialogResult.Primary)
        {
            return;
        }
        // A picture the core cannot read as one is attached rather than dropped on the floor: the
        // user asked for it to be in the message, and losing it silently is the worse answer.
        var unreadable = new List<StorageFile>();
        foreach (var picture in pictures)
        {
            try
            {
                var dataUrl = MailcalBindingsMethods.ComposerImageDataUrl(picture.Path);
                var payload = JsonSerializer.Serialize(
                    new Dictionary<string, string> { ["data_url"] = dataUrl, ["file_name"] = picture.Name });
                await _editor.RunAsync($"window.insertComposerImage({EditorWebViewHost.Arg(payload)})");
            }
            catch (Exception ex)
            {
                Log.Info($"composer: a dropped file could not be shown in the message ({ex.GetType().Name})");
                unreadable.Add(picture);
            }
        }
        if (unreadable.Count > 0)
        {
            ShowError(L10n.ComposeImageFailed());
            Attach(unreadable);
        }
    }
}
