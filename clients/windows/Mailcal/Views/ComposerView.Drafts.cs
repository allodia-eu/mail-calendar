// Keeping the composer's message on the server: the composition it saves under, the idle timer
// that stores it, and the hint under the editor (docs/drafts.md).
//
// A partial of ComposerView, its own file because ComposerView.xaml.cs is near the 500-line limit.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    /// <summary>This composer's composition, for as long as it is open: the host's handle on one
    /// composer, and what every draft call names. Empty while nothing is bound.</summary>
    private string _composition = string.Empty;

    /// <summary>The idle interval, restarted on every change, so the trigger is the pause and not
    /// the clock. One shot: it stops itself on the tick that stores the draft.</summary>
    private DispatcherTimer? _draftIdle;

    /// <summary>Watches the editor for changes this host cannot otherwise see. Every header field
    /// raises its own event; the message body is a WebView2 and the page has no channel back, so
    /// the shared bundle counts its own mutations and this reads the count.</summary>
    private DispatcherTimer? _draftSampler;

    /// <summary>The revision this host last saw. <c>-1</c> until the bundle has parsed, so a page
    /// still loading is never mistaken for one that has just been typed in.</summary>
    private int _seenRevision = -1;

    /// <summary>Whether this composer is keeping a draft: false before it is bound, and again
    /// once it has been torn down, so a second teardown forgets nothing twice.</summary>
    private bool _keepsDraft;

    /// <summary>
    /// Binds the draft half: the composition this composer saves under, the Save control, the
    /// hint, and the two timers.
    /// </summary>
    /// <remarks>
    /// A <b>resumed</b> draft arrives with its composition, because the core has already joined
    /// that id to the copy on the server; a fresh one here would store a second draft beside the
    /// one the composer is showing. Every other composer mints its own.
    /// </remarks>
    private void InitDrafts(MailboxModel model, ComposeContext request)
    {
        _composition = request.Composition ?? Guid.NewGuid().ToString();
        _keepsDraft = true;
        SaveDraftButton.Visibility = Visibility.Visible;
        model.DraftStatusChanged += OnDraftStatusChanged;

        var idle = TimeSpan.FromSeconds(MailcalBindingsMethods.DraftAutosaveIdleSeconds());
        _draftIdle = new DispatcherTimer { Interval = idle };
        _draftIdle.Tick += (_, _) =>
        {
            _draftIdle!.Stop();
            _ = SaveDraftAsync();
        };
        // A third of the interval, which is what the lag costs: a draft reaches the server between
        // one and one-and-a-third intervals after the last keystroke, and never during typing.
        _draftSampler = new DispatcherTimer
        {
            Interval = TimeSpan.FromSeconds(Math.Max(1.0, idle.TotalSeconds / 3)),
        };
        _draftSampler.Tick += async (_, _) => await SampleEditorAsync();
        _draftSampler.Start();
    }

    /// <summary>Notes that something in the message changed, and starts the interval again.</summary>
    private void NoteDraftChange()
    {
        if (!_keepsDraft)
        {
            return;
        }
        _draftIdle!.Stop();
        _draftIdle.Start();
    }

    /// <summary>Reads the editor's change count, and calls the composer changed when it moves.</summary>
    private async Task SampleEditorAsync()
    {
        if (!_editorReady)
        {
            return;
        }
        try
        {
            var revision = await _editor.ReadNumberAsync("window.composerRevision()");
            if (revision < 0 || revision == _seenRevision)
            {
                return;
            }
            // The first reading is the baseline the editor loaded with, never an edit: without it
            // every composer would store a draft moments after opening.
            if (_seenRevision >= 0)
            {
                NoteDraftChange();
            }
            _seenRevision = revision;
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't read the editor's change count ({ex.GetType().Name})");
        }
    }

    private void OnSaveDraft(object sender, RoutedEventArgs e) => _ = SaveDraftAsync();

    /// <summary>
    /// Stores the draft now, whatever the idle timer is doing.
    /// </summary>
    /// <remarks>
    /// Both triggers land here and the core cannot tell them apart, which is deliberate: pressing
    /// Save on an unchanged draft reaches no server, exactly as an idle timer firing on an
    /// untouched composer does.
    /// <para>A document that could not be read is logged and dropped rather than shown. Saving is
    /// never something the user waits for, and never something a composer refuses to be dismissed
    /// over; a save the <i>server</i> refuses is reported, through the hint.</para>
    /// </remarks>
    private async Task SaveDraftAsync()
    {
        if (!_keepsDraft || _model is null)
        {
            return;
        }
        try
        {
            var documentJson = await ReadDocumentAsync();
            if (string.IsNullOrEmpty(documentJson))
            {
                return;
            }
            _model.SaveDraft(
                _composition,
                new Recipients(ToField.Text, CcField.Text, BccField.Text),
                SubjectBox.Text,
                documentJson,
                _attachments.Select(a => a.File).ToArray(),
                (FromBox.SelectedItem as AccountItem)?.Id);
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't prepare the draft to save ({ex.GetType().Name})");
        }
    }

    /// <summary>
    /// Removes this composer's stored draft from the server.
    /// </summary>
    /// <remarks>
    /// The one path that does: closing the composer any other way leaves the draft in Drafts,
    /// which is what a resumed draft the user only looked at needs (docs/drafts.md).
    /// </remarks>
    internal void DiscardStoredDraft()
    {
        if (_keepsDraft)
        {
            _model?.DiscardDraft(_composition);
        }
    }

    /// <summary>
    /// Forgets the composition, leaving the stored draft where it is.
    /// </summary>
    /// <remarks>
    /// Called from <c>Teardown</c>, so it runs however the composer went, sent, discarded or
    /// dismissed. Without it the core holds a record per composer for the life of the process.
    /// </remarks>
    private void TeardownDrafts()
    {
        _draftIdle?.Stop();
        _draftSampler?.Stop();
        if (!_keepsDraft)
        {
            return;
        }
        if (_model is not null)
        {
            _model.DraftStatusChanged -= OnDraftStatusChanged;
            _model.CloseComposition(_composition);
        }
        _keepsDraft = false;
    }

    /// <summary>Re-pulls this composition's own state. The signal names no composition, so every
    /// open composer asks for its own.</summary>
    private void OnDraftStatusChanged()
    {
        if (!_keepsDraft || _model is null)
        {
            return;
        }
        ShowDraftHint(_model.DraftStatusOf(_composition));
    }

    /// <summary>The quiet line under the editor. A hint and never a gate: no state here stops the
    /// composer being closed, and none of it is worth a dialog. A composer that has saved nothing
    /// says nothing.</summary>
    private void ShowDraftHint(DraftStatus status)
    {
        var text = status switch
        {
            DraftStatus.Saving => L10n.ComposeDraftSaving(),
            DraftStatus.Saved => L10n.ComposeDraftSaved(),
            DraftStatus.Queued => L10n.ComposeDraftQueued(),
            DraftStatus.Failed => L10n.ComposeDraftFailed(),
            _ => null,
        };
        if (text is null)
        {
            DraftHint.Visibility = Visibility.Collapsed;
            return;
        }
        DraftHint.Text = text;
        DraftHint.Foreground = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources[
            status == DraftStatus.Failed
                ? "SystemFillColorCriticalBrush"
                : "TextFillColorSecondaryBrush"];
        DraftHint.Visibility = Visibility.Visible;
    }
}
