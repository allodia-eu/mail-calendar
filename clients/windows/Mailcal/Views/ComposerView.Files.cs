// The composer's files. The core holds them (docs/composer-layout.md, rule 3): the picker and a
// drop add to them, a tile's Remove takes one off, and every change draws the tiles again from the
// list the core answers with. Attach is drawn from the same answer, running while files the person
// chose are still being read (docs/waiting.md).

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.Windows.Storage.Pickers;
using uniffi.mailcal_bindings;
using Windows.Storage;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>What a composer the core cannot answer for draws: no files, and Attach free. The
    /// total size has a default, so the C# binding puts it last and it is left out here.</summary>
    private static readonly ComposerFiles NoFiles = new([], false, new ControlState(true, false));

    /// <summary>The list as the core last answered it: what the tiles show, and the files' half of
    /// whether anything was written (<see cref="ComposerFiles.Changed"/>).</summary>
    private ComposerFiles? _files;

    private async void AttachFromPicker()
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
        if (results.Count == 0)
        {
            return;
        }
        StartAttaching();
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
        await FinishAttachingAsync(picked);
    }

    /// <summary>Hands <paramref name="files"/>, which are ready, to the core's list and draws what
    /// it answers, Attach running while the core reads them.</summary>
    private async Task AttachAsync(IReadOnlyList<StorageFile> files)
    {
        if (files.Count == 0)
        {
            return;
        }
        StartAttaching();
        await FinishAttachingAsync(files);
    }

    /// <summary>Starts an addition: Attach runs until <see cref="FinishAttachingAsync"/> ends it,
    /// which is called exactly once for each start.</summary>
    private void StartAttaching()
    {
        if (_model is { } model)
        {
            ShowFiles(model.BeginAddingComposerFiles(_composition));
        }
    }

    /// <summary>Ends one addition with <paramref name="files"/>, none when nothing arrived.</summary>
    /// <remarks>Off the UI thread: the core reads each file's size, and a file may sit on a network
    /// share. Draws the list read afterwards rather than the call's own answer, because two
    /// additions can end in either order and the one drawn last must be whole.</remarks>
    private async Task FinishAttachingAsync(IEnumerable<StorageFile> files)
    {
        if (_model is not { } model)
        {
            return;
        }
        var attachments = files
            .Select(file => new ComposerFileAttachment(file.Path, file.Name, MediaType(file)))
            .ToList();
        var composition = _composition;
        await Task.Run(() => model.FinishAddingComposerFiles(composition, attachments));
        ShowFiles(model.ComposerFilesOf(composition));
    }

    private void RemoveFile(string file)
    {
        if (_model is not null)
        {
            ShowFiles(_model.RemoveComposerFile(_composition, file));
        }
    }

    /// <summary>Draws one tile per file, and Attach. A call that failed answers <c>null</c> and
    /// leaves what is drawn, which is still the core's last answer.</summary>
    private void ShowFiles(ComposerFiles? files)
    {
        if (files is null)
        {
            return;
        }
        _files = files;
        AttachHost.Children.Clear();
        AttachHost.Children.Add(ControlComponents.Button(
            L10n.ActionAttach(), files.Attach, "ComposerAttach", AttachFromPicker));
        FileTiles.ItemsSource = files.Tiles
            .Select(tile => new ComposerFileTileView(tile, () => RemoveFile(tile.Id)))
            .ToList();
        FilesSummary.Text = L10n.ComposeFilesSummary(files.Tiles.Length, ByteSize.Format(files.TotalSize));
        FilesBlock.Visibility = files.Tiles.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    private static string MediaType(StorageFile file) =>
        string.IsNullOrWhiteSpace(file.ContentType) ? "application/octet-stream" : file.ContentType;
}
