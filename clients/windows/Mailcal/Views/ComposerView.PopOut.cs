// Popping the composer out: the reading pane's composer moved into a window of its own, holding
// exactly what it held (docs/reading-window.md, "Popping a composer out").
//
// A partial of ComposerView, its own file because ComposerView.xaml.cs is near the 500-line limit.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    private Func<ComposerState, Task<bool>>? _popOut;

    /// <summary>Moves this composer into a window of its own, answering whether it moved; set by
    /// the shell for the reading pane's composer, and left unset in a composer window, which hides
    /// the button.</summary>
    internal Func<ComposerState, Task<bool>>? PopOut
    {
        get => _popOut;
        set
        {
            _popOut = value;
            PopOutButton.Visibility = value is null ? Visibility.Collapsed : Visibility.Visible;
        }
    }

    /// <summary>
    /// Reads everything the composer holds and hands it to the shell to open in a window.
    /// </summary>
    /// <remarks>
    /// The composition is the window's from here on, so this composer marks itself finished
    /// <b>before</b> the shell tears it down: teardown would otherwise forget the composition the
    /// window has just opened on, and a pending autosave would save from a composer that is gone.
    /// A move that did not happen takes it back.
    /// </remarks>
    private async void OnPopOut(object sender, RoutedEventArgs e)
    {
        if (_popOut is not { } popOut || _model is null || _finished)
        {
            return;
        }
        PopOutButton.IsEnabled = false;
        try
        {
            var documentJson = await ReadDocumentAsync();
            if (string.IsNullOrEmpty(documentJson) || _finished)
            {
                ShowError(L10n.ComposeOpenFailed());
                return;
            }
            var state = new ComposerState(
                (FromBox.SelectedItem as AccountItem)?.Id,
                ToField.Text,
                CcField.Text,
                BccField.Text,
                SubjectBox.Text,
                documentJson,
                _attachments.Select(a => a.File).ToArray(),
                MovedSignatureNow(),
                await IsDirtyAsync());
            _draftIdle?.Stop();
            _draftSampler?.Stop();
            _finished = true;
            if (!await popOut(state))
            {
                _finished = false;
                _draftSampler?.Start();
                ShowError(L10n.ComposeOpenFailed());
            }
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't move to a window ({ex.GetType().Name})");
            ShowError(L10n.ComposeOpenFailed());
        }
        finally
        {
            PopOutButton.IsEnabled = true;
        }
    }

    /// <summary>What this composer is doing with the signature, as a moved one carries it on.</summary>
    private MovedSignature MovedSignatureNow()
    {
        if (_request?.ManagesSignature != true)
        {
            return new MovedSignature.Untouched();
        }
        if (_signatureChoice is null)
        {
            return new MovedSignature.FollowsAccount();
        }
        return _signatureChoice.Id is { } id
            ? new MovedSignature.Chosen(id)
            : new MovedSignature.NoSignature();
    }
}
