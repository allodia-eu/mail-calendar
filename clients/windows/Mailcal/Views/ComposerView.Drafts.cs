// Keeping the composer's message on the server: the composition it saves under, the idle timer
// that stores it, the hint under the editor, and the two ways out, leaving and Discard
// (docs/drafts.md).
//
// A partial of ComposerView, its own file because ComposerView.xaml.cs is near the 500-line limit.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
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

    /// <summary>Whether the core has already been told how this composition ends: its message was
    /// submitted, it was left with a save, or it was discarded. Teardown then forgets nothing.</summary>
    /// <remarks>
    /// After a submit the send owns the composition and finishes with it when the message settles,
    /// so forgetting it here as well would race the cleanup that takes the stored draft away: the
    /// two are separate tasks, and this one landing first leaves the draft in Drafts for ever
    /// (docs/drafts.md). A leave and a discard each forget it themselves.
    /// </remarks>
    private bool _finished;

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

    /// <summary>
    /// Reads the count the editor holds once it is seeded, as the baseline every later sample is
    /// compared against.
    /// </summary>
    /// <remarks>
    /// Read here rather than on the sampler's first tick, which comes a third of the interval
    /// later: anything typed before that tick would be taken into the baseline, and a short reply
    /// written straight after opening would never be saved. A failure leaves the baseline to the
    /// first tick.
    /// </remarks>
    private async Task BaselineEditorRevisionAsync()
    {
        try
        {
            var revision = await _editor.ReadNumberAsync("window.composerRevision()");
            if (revision >= 0)
            {
                _seenRevision = revision;
            }
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't read the editor's change count ({ex.GetType().Name})");
        }
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
            // With no baseline from the seed, the first reading is the baseline, never an edit:
            // without one every composer would store a draft moments after opening.
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
            // Left or discarded while the document was being read: a save now would arrive after
            // the composition was forgotten and store a second copy.
            if (string.IsNullOrEmpty(documentJson) || _finished)
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
    /// Leaves the composer: stores what was written in it and forgets the composition, or only
    /// lets teardown forget it when nothing was. Never asks (docs/drafts.md, "Leaving a composer").
    /// </summary>
    /// <remarks>
    /// The save and the forget are one call to the core, so teardown must not close the
    /// composition again afterwards. The timers stop first, so an autosave cannot land after the
    /// composition is forgotten. A document that cannot be read is logged and the composer closes
    /// on its last save, on the footing a failed autosave is on.
    /// </remarks>
    internal async Task LeaveAsync()
    {
        if (!_keepsDraft || _finished || _model is null)
        {
            return;
        }
        _draftIdle?.Stop();
        _draftSampler?.Stop();
        if (ComposerExit.OnLeave(_keepsDraft, await IsDirtyAsync()) != ComposerLeave.SaveAndClose)
        {
            return;
        }
        try
        {
            var documentJson = await ReadDocumentAsync();
            if (string.IsNullOrEmpty(documentJson) || _finished)
            {
                return;
            }
            _model.SaveDraftAndClose(
                _composition,
                new Recipients(ToField.Text, CcField.Text, BccField.Text),
                SubjectBox.Text,
                documentJson,
                _attachments.Select(a => a.File).ToArray(),
                (FromBox.SelectedItem as AccountItem)?.Id);
            _finished = true;
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't prepare the draft to keep on leaving ({ex.GetType().Name})");
        }
    }

    /// <summary>
    /// The composer's Discard button: the one way to throw a draft away, so it asks first whenever
    /// something would be lost, and otherwise closes at once.
    /// </summary>
    /// <remarks>
    /// "Keep editing" rather than the helper's default "Cancel" beside "Discard": a button labelled
    /// Cancel reads ambiguously as "cancel the draft".
    /// </remarks>
    private async void OnDiscard(object sender, RoutedEventArgs e)
    {
        var stored = _keepsDraft && _model is not null && _model.DraftIsStored(_composition);
        if (ComposerExit.AsksBeforeDiscard(await IsDirtyAsync(), stored))
        {
            var answer = await DialogHelper.ConfirmAsync(
                XamlRoot,
                L10n.ComposeDiscardTitle(),
                L10n.ComposeDiscardMessage(),
                L10n.ActionDiscard(),
                L10n.ActionKeepEditing());
            if (answer != ContentDialogResult.Primary)
            {
                return;
            }
        }
        if (_keepsDraft && !_finished)
        {
            // Removes the stored copy and forgets the composition, so teardown has nothing left to
            // forget. A composition that never saved reaches no server.
            _model?.DiscardDraft(_composition);
            _finished = true;
        }
        _onDone?.Invoke();
    }

    /// <summary>
    /// Forgets the composition, leaving the stored draft where it is.
    /// </summary>
    /// <remarks>
    /// Called from <c>Teardown</c>, so it runs however the composer went, and stops the timers
    /// either way. The composition is forgotten here only when nothing else has finished it: a
    /// submit, a leave and a discard each did (see <see cref="_finished"/>).
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
            if (!_finished)
            {
                _model.CloseComposition(_composition);
            }
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
