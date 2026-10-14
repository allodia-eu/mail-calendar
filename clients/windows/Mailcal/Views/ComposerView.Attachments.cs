// Files the user attaches through the picker, and the list that shows them.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Windows.Storage.Pickers;
using uniffi.mailcal_bindings;
using Windows.Storage;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    private async void OnAttachFiles(object sender, RoutedEventArgs e)
    {
        IReadOnlyList<PickFileResult> results;
        try
        {
            var picker = new FileOpenPicker(PickerOwner.For(this));
            picker.FileTypeFilter.Add("*");
            results = await picker.PickMultipleFilesAsync();
        }
        catch (Exception ex)
        {
            // An async void handler: a throw here would take the app down with it.
            Log.Warn($"composer: attach picker failed ({ex.GetType().Name})");
            return;
        }
        foreach (var picked in results)
        {
            if (string.IsNullOrEmpty(picked.Path))
            {
                continue;
            }
            // Opened as a StorageFile for the media type the system knows it by; the core reads
            // the bytes from the path when the message is sent.
            StorageFile file;
            try
            {
                file = await StorageFile.GetFileFromPathAsync(picked.Path);
            }
            catch (Exception ex)
            {
                Log.Warn($"composer: a picked file is unreadable ({ex.GetType().Name})");
                continue;
            }
            _attachments.Add(new PickedComposerAttachment(
                new ComposerFileAttachment(file.Path, file.Name, MediaType(file))));
        }
        AttachmentList.ItemsSource = null;
        AttachmentList.ItemsSource = _attachments;
    }

    private void OnRemoveAttachment(object sender, RoutedEventArgs e)
    {
        if (AttachmentList.SelectedItem is not PickedComposerAttachment selected)
        {
            return;
        }
        _attachments.Remove(selected);
        AttachmentList.ItemsSource = null;
        AttachmentList.ItemsSource = _attachments;
        RemoveAttachmentButton.IsEnabled = false;
    }

    private void OnAttachmentSelectionChanged(object sender, SelectionChangedEventArgs e) =>
        RemoveAttachmentButton.IsEnabled = AttachmentList.SelectedItem is not null;

    private static string MediaType(StorageFile file) =>
        string.IsNullOrWhiteSpace(file.ContentType) ? "application/octet-stream" : file.ContentType;

    private sealed class PickedComposerAttachment
    {
        public PickedComposerAttachment(ComposerFileAttachment file) => File = file;

        public ComposerFileAttachment File { get; }

        public string DisplayName => File.FileName;
    }
}
