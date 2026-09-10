// The reading pane's overflow menu, at the end of the action row, and the .eml export behind it
// (docs/reading-actions.md). Split out of ReadingView.xaml.cs to keep that file under the
// 500-line limit.
//
// The file written is the message exactly as it was delivered: the core hands over the raw
// source the engine cached, never a rebuild of what this pane renders.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    // Which message the standing export error is about, so Render() can tell "the user moved on"
    // from "another snapshot arrived for the message still on screen". Null while none is shown.
    private string? _exportErrorKey;

    private async void OnExportEml(object sender, RoutedEventArgs e)
    {
        if (_model?.OpenedMessage is not { } opened)
        {
            return;
        }
        ClearExportError();
        StorageFile? file;
        try
        {
            // SubjectText.Text, not OpenedMessage.Subject: the contract is that the name comes
            // from the subject this pane DISPLAYS, so an untitled message exports under the
            // client's own localised "(no subject)" rather than a second, English name
            // (docs/reading-actions.md). Read off the control the user is looking at rather than
            // recomputing the placeholder here, which would be a second answer free to drift from
            // the one on screen.
            var picker = new FileSavePicker
            {
                SuggestedFileName = MailboxModel.ExportFileName(SubjectText.Text),
            };
            // The extension names its own type here rather than a catalog string: a picker
            // filter is the one label that is an identifier, and ".eml" reads the same in every
            // language the app ships.
            picker.FileTypeChoices.Add(".eml", new List<string> { ".eml" });
            if (App.MainWindow is not null)
            {
                InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(App.MainWindow));
            }
            file = await picker.PickSaveFileAsync();
        }
        catch (Exception ex)
        {
            Log.Warn($"message export picker failed: {ex.GetType().Name}");
            ShowExportError(opened.Key);
            return;
        }
        if (file is null)
        {
            return;
        }
        // Written to an app-owned temp file first and copied in, the same route the attachment
        // save takes: the core writes by path, and the path a picker returns is not one this
        // process may always open for writing.
        var temp = Path.Combine(Path.GetTempPath(), $"mailcal-export-{Guid.NewGuid():N}.eml");
        var ok = false;
        try
        {
            if (await WriteSourceAsync(opened.Account, opened.Key, temp))
            {
                await using var input = File.OpenRead(temp);
                await using var output = await file.OpenStreamForWriteAsync();
                output.SetLength(0);
                await input.CopyToAsync(output);
                ok = true;
            }
        }
        catch (Exception ex)
        {
            Log.Warn($"message export failed: {ex.GetType().Name}");
        }
        finally
        {
            DeleteQuietly(temp);
        }
        if (!ok)
        {
            // Never leave a half-written .eml at the user's chosen path: it would open in another
            // mail client as a damaged message rather than as nothing at all.
            try
            {
                await file.DeleteAsync();
            }
            catch
            {
                // best effort
            }
            ShowExportError(opened.Key);
        }
    }

    // Writes the raw source on a background thread: it may not be cached, and fetching it must
    // not block the UI.
    private Task<bool> WriteSourceAsync(string account, string key, string destination) =>
        Task.Run(() => _model?.SaveMessageSource(account, key, destination) ?? false);

    private void ShowExportError(string key)
    {
        _exportErrorKey = key;
        ExportError.Text = L10n.MessageSaveFailed();
        ExportError.Visibility = Visibility.Visible;
    }

    private void ClearExportError()
    {
        _exportErrorKey = null;
        ExportError.Visibility = Visibility.Collapsed;
    }
}
