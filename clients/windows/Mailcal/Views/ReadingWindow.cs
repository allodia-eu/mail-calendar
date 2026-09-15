// A message in a window of its own (docs/reading-window.md).
//
// Everything the window draws is ReadingView, the same view the reading pane draws, reading a slot
// of its own in the core. What this file adds is the three things only a window has: where its body
// comes from, what its action row does when there is no pane to fall back to, and telling the core
// to drop the body once the window has gone.
//
// A window is a VIEW OF THE RUNNING APP, never a second instance of it: it is handed the shell's
// one MailboxModel, so there is one core over one store however many windows are open.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Windows.Graphics;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>One detached reading window: a full reading view over one message.</summary>
internal sealed class ReadingWindow : Window
{
    private readonly MailboxModel _model;
    private readonly WindowReader _reader;
    private readonly ReadingView _view = new();

    /// <summary>The shell's own default, so a window opens wide enough for the whole action row
    /// rather than at whatever size the toolkit picks.</summary>
    private const int DefaultWidth = 760;
    private const int DefaultHeight = 620;

    /// <summary>Opens a window reading <paramref name="reader"/>'s slot.</summary>
    /// <remarks>
    /// The window is built around the reader the model already minted, never around a fresh open:
    /// the fetch, the bounded retry, the loading threshold and the mark-read all belong to the one
    /// open the model dispatched, so there is no second path to a body here.
    /// </remarks>
    internal ReadingWindow(MailboxModel model, WindowReader reader)
    {
        _model = model;
        _reader = reader;

        // Named after the message, so two open windows are distinguishable in the window list the
        // OS draws. The icon and the opening size come with the title rather than beside it, so a
        // window cannot be given one and not the others.
        WindowChrome.Dress(
            this,
            _view,
            reader.Opened is { Subject.Length: > 0 } opened ? opened.Subject : L10n.MailNoSubject(),
            new SizeInt32(DefaultWidth, DefaultHeight));
        AppearanceApplied(model.CurrentAppearance);

        _view.Init(model, reader, WindowActions());
        Closed += OnClosed;
    }

    /// <summary>The core reader id this window's body arrives in, so the shell can tell whether a
    /// message already has a window.</summary>
    internal string ReaderId => _reader.Id;

    /// <summary>Repaints this window when the app's appearance changes under it.</summary>
    internal void AppearanceApplied(Appearance appearance)
    {
        if (Content is FrameworkElement root)
        {
            root.RequestedTheme = MainWindow.Theme(appearance);
        }
    }

    // Reply, reply-all and forward open a COMPOSER WINDOW rather than the shell's inline composer:
    // a draft answering the window in front of you does not belong in a different window behind it.
    // Archive and delete CLOSE this window, because the message has left the folder and there is
    // nothing left for the window to be about; the pane advances to the next message down instead,
    // being a place in a list, but a window is one message.
    private ReadingActions WindowActions() => new(
        (opened, all) => App.Shell?.ComposeReplyInWindow(opened, _reader.Body, all),
        opened => App.Shell?.ComposeForwardInWindow(opened, _reader.Body),
        opened =>
        {
            _model.Archive(opened.Account, opened.Key);
            Close();
        },
        opened =>
        {
            _model.Delete(opened.Account, opened.Key);
            Close();
        });

    // Both halves of the close: the shell forgets the window, the model forgets the reader and
    // tells the core to drop the body it was holding, and the view releases its browser process.
    // A sanitised body carries every inline image resolved into it, so it is the largest thing
    // either side holds per window.
    private void OnClosed(object sender, WindowEventArgs args)
    {
        App.Shell?.ForgetReadingWindow(this);
        _model.CloseReadingWindow(_reader.Id);
        _view.Teardown();
    }
}
