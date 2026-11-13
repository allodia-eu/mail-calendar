// The composer's files. The core holds them (docs/composer-layout.md, rule 3): the picker and a
// drop add to them, a tile's Remove takes one off, and every change draws the tiles again from the
// list the core answers with.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.Windows.Storage.Pickers;
using uniffi.mailcal_bindings;
using Windows.Storage;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>The list as the core last answered it: what the tiles show, and the files' half of
    /// whether anything was written (<see cref="ComposerFiles.Changed"/>).</summary>
    private ComposerFiles? _files;

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
        var picked = new List<StorageFile>();
        foreach (var result in results)
        {
            if (string.IsNullOrEmpty(result.Path))
            {
                continue;
            }
            // Opened as a StorageFile for the media type the system knows it by; the core reads
            // the bytes from the path when the message is saved or sent.
            try
            {
                picked.Add(await StorageFile.GetFileFromPathAsync(result.Path));
            }
            catch (Exception ex)
            {
                Log.Warn($"composer: a picked file is unreadable ({ex.GetType().Name})");
            }
        }
        await AttachAsync(picked);
    }

    /// <summary>Hands <paramref name="files"/> to the core's list and draws what it answers.</summary>
    /// <remarks>Off the UI thread: the core reads each file's size, and a file may sit on a network
    /// share.</remarks>
    private async Task AttachAsync(IEnumerable<StorageFile> files)
    {
        var attachments = files
            .Select(file => new ComposerFileAttachment(file.Path, file.Name, MediaType(file)))
            .ToList();
        if (_model is not { } model || attachments.Count == 0)
        {
            return;
        }
        var composition = _composition;
        ShowFiles(await Task.Run(() => model.AddComposerFiles(composition, attachments)));
    }

    private void RemoveFile(string file)
    {
        if (_model is not null)
        {
            ShowFiles(_model.RemoveComposerFile(_composition, file));
        }
    }

    /// <summary>Draws one tile per file. A call that failed answers <c>null</c> and leaves what is
    /// drawn, which is still the core's last answer.</summary>
    private void ShowFiles(ComposerFiles? files)
    {
        if (files is null)
        {
            return;
        }
        _files = files;
        FileTiles.ItemsSource = files.Tiles
            .Select(tile => new ComposerFileTileView(tile, () => RemoveFile(tile.Id)))
            .ToList();
        FileTiles.Visibility = files.Tiles.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    private static string MediaType(StorageFile file) =>
        string.IsNullOrWhiteSpace(file.ContentType) ? "application/octet-stream" : file.ContentType;
}
