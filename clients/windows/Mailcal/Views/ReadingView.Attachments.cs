// The reading pane's attachment strip: listing the open message's files, saving one to a
// user-chosen path, and opening one via the OS default handler. Split out of ReadingView.xaml.cs
// to keep that file under the 500-line limit.
//
// We never render or execute attachment content in-app: the core decodes the part to an app-owned
// temp file and the OS takes it from there (running its own file scanning). See
// docs/rendering-security.md, attachment bytes are hostile input like any other part of a message.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    private void SetAttachments(ReadingBody body)
    {
        AttachmentItems.ItemsSource = body.Attachments;
        AttachmentPanel.Visibility = body.Attachments.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        // Clear any stale save/open error from a previous message.
        AttachmentError.Visibility = Visibility.Collapsed;
    }

    private async void OnSaveAttachment(object sender, RoutedEventArgs e)
    {
        if (_model?.OpenedMessage is not { } opened
            || (sender as FrameworkElement)?.DataContext is not MessageAttachment attachment
            || !TryBeginBusy(sender as Button, out var endBusy))
        {
            return;
        }
        try
        {
            StorageFile? file;
            try
            {
                var picker = new FileSavePicker
                {
                    SuggestedFileName = string.IsNullOrEmpty(attachment.FileName)
                        ? "attachment"
                        : attachment.FileName,
                };
                // ExtensionFor sanitises the value, an unsanitized Path.GetExtension can carry
                // spaces/parens (e.g. "report.final draft") that make FileTypeChoices throw.
                picker.FileTypeChoices.Add(
                    L10n.AttachmentsTitle(),
                    new List<string> { ExtensionFor(attachment.FileName) });
                if (App.MainWindow is not null)
                {
                    InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(App.MainWindow));
                }
                file = await picker.PickSaveFileAsync();
            }
            catch (Exception ex)
            {
                Log.Warn($"attachment save picker failed: {ex.GetType().Name}");
                ShowAttachmentError(L10n.AttachmentSaveFailed());
                return;
            }
            if (file is null)
            {
                return;
            }
            var temp = Path.Combine(Path.GetTempPath(), $"mailcal-attachment-{Guid.NewGuid():N}.part");
            var ok = false;
            try
            {
                if (await DecodeToAsync(opened.Account, opened.Key, attachment.Id, temp))
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
                // A mid-write failure (disk full, disconnected target) must not crash the app.
                Log.Warn($"attachment save failed: {ex.GetType().Name}");
            }
            finally
            {
                DeleteQuietly(temp);
            }
            if (!ok)
            {
                // Don't leave a half-written or empty file at the user's chosen path.
                try
                {
                    await file.DeleteAsync();
                }
                catch
                {
                    // best effort
                }
                ShowAttachmentError(L10n.AttachmentSaveFailed());
            }
        }
        finally
        {
            endBusy();
        }
    }

    private async void OnOpenAttachment(object sender, RoutedEventArgs e)
    {
        if (_model?.OpenedMessage is not { } opened
            || (sender as FrameworkElement)?.DataContext is not MessageAttachment attachment
            || !TryBeginBusy(sender as Button, out var endBusy))
        {
            return;
        }
        // Decode to an app-owned temp file, then hand it to the OS default handler (which runs
        // the OS's own file scanning). We never render or execute attachment content in-app.
        var dir = Path.Combine(Path.GetTempPath(), "mailcal-opened");
        var temp = Path.Combine(dir, $"{Guid.NewGuid():N}{ExtensionFor(attachment.FileName)}");
        try
        {
            Directory.CreateDirectory(dir);
            if (!await DecodeToAsync(opened.Account, opened.Key, attachment.Id, temp))
            {
                ShowAttachmentError(L10n.AttachmentOpenFailed());
                return;
            }
            var file = await StorageFile.GetFileFromPathAsync(temp);
            if (!await Windows.System.Launcher.LaunchFileAsync(file))
            {
                ShowAttachmentError(L10n.AttachmentOpenFailed());
            }
        }
        catch (Exception ex)
        {
            Log.Warn($"attachment open failed: {ex.GetType().Name}");
            ShowAttachmentError(L10n.AttachmentOpenFailed());
        }
        finally
        {
            endBusy();
        }
    }

    // Swap a clicked attachment button into a spinner for the duration of its open/save, so a
    // brief delay, the OS launching the default handler, the save picker, or decoding a large
    // part off the UI thread, reads as "working" rather than a dead click. Returns false when
    // the button is already busy, collapsing a double-click into one action; endBusy() restores
    // the label. The button holds its width (no row reflow) and ignores further pointer input
    // while busy, rather than going disabled, which would dim the spinning ring.
    private static bool TryBeginBusy(Button? button, out Action endBusy)
    {
        endBusy = static () => { };
        if (button is null || button.Content is ProgressRing)
        {
            return false;
        }
        var label = button.Content;
        button.MinWidth = button.ActualWidth;
        button.Content = new ProgressRing { IsActive = true, Width = 16, Height = 16 };
        button.IsHitTestVisible = false;
        endBusy = () =>
        {
            button.Content = label;
            button.IsHitTestVisible = true;
            button.ClearValue(FrameworkElement.MinWidthProperty);
        };
        return true;
    }

    // Decodes one attachment to a filesystem path on a background thread, the core decodes and
    // writes the whole part synchronously, so keep it off the UI thread for large attachments.
    private Task<bool> DecodeToAsync(string account, string key, uint attachmentId, string destination) =>
        Task.Run(() => _model?.SaveAttachment(account, key, attachmentId, destination) ?? false);

    private void ShowAttachmentError(string message)
    {
        AttachmentError.Text = message;
        AttachmentError.Visibility = Visibility.Visible;
    }

    private static void DeleteQuietly(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
        }
        catch
        {
            // best effort, a leftover temp file is harmless
        }
    }

    // A picker-safe file extension for the save/open temp name. Path.GetExtension can return a
    // value with spaces or other characters that FileSavePicker.FileTypeChoices rejects (which
    // would otherwise throw), so keep the dot plus the leading run of ASCII letters/digits.
    private static string ExtensionFor(string fileName)
    {
        var extension = Path.GetExtension(fileName);
        if (string.IsNullOrEmpty(extension) || extension.Length < 2)
        {
            return ".bin";
        }
        var cleaned = new string(extension.Skip(1).TakeWhile(char.IsLetterOrDigit).ToArray());
        return cleaned.Length == 0 ? ".bin" : "." + cleaned;
    }
}
