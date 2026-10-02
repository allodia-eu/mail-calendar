// A draft in a window of its own (docs/reading-window.md), raised by replying or forwarding from
// inside a reading window.
//
// It hosts ComposerView, the same composer the shell renders in the reading pane's column, built
// from the same ComposeContext: so a reply raised in a window is seeded, signed, submitted and
// discarded by the paths that already existed. What this file adds is the window around it.
//
// THE MAIN WINDOW'S COMPOSER DOES NOT MOVE. A reply raised in the reading pane still replaces that
// pane, which is a shipped capability; this is beside it, not instead of it.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Windows.Graphics;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>One detached composer window: a full composer over one draft.</summary>
internal sealed class ComposerWindow : Window
{
    private readonly ComposerView _view = new();

    /// <summary>Whether the composer has been left on the way to a close the user asked for, so
    /// the close that follows goes through.</summary>
    private bool _left;

    /// <summary>Roomy enough for the recipient fields and the editor without a scroll.</summary>
    private const int DefaultWidth = 780;
    private const int DefaultHeight = 680;

    /// <summary>Opens a window on <paramref name="request"/>.</summary>
    internal ComposerWindow(MailboxModel model, ComposeContext request)
    {
        // Named after the DRAFT, so two open drafts are distinguishable in the window list the OS
        // draws. The subject the composer opens with, falling back to what it is doing when there
        // is none: a forward of an unsubjected message would otherwise be a window called nothing.
        WindowChrome.Dress(
            this,
            _view,
            string.IsNullOrWhiteSpace(request.InitialSubject) ? request.Title : request.InitialSubject,
            new SizeInt32(DefaultWidth, DefaultHeight));
        AppearanceApplied(model.CurrentAppearance);

        // Send and Discard both finish the draft, and both close the window: the composer has no
        // pane to give back here.
        _view.Init(model, request, Close);
        AppWindow.Closing += OnClosing;
        Closed += OnClosed;
    }

    /// <summary>Repaints this window when the app's appearance changes under it.</summary>
    internal void AppearanceApplied(Appearance appearance)
    {
        if (Content is FrameworkElement root)
        {
            root.RequestedTheme = MainWindow.Theme(appearance);
        }
    }

    // CLOSING THE WINDOW IS LEAVING THE COMPOSER: what was written in it is kept in Drafts and
    // nobody is asked (docs/drafts.md, "Leaving a composer"). The close is held while the composer
    // reads its document, which needs the editor's WebView alive, and then let go. Raised only for
    // a close through the window's own chrome or the system, never for the Close() that Send and
    // Discard call, which have already finished the composition.
    private async void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_left)
        {
            return;
        }
        args.Cancel = true;
        _left = true;
        await _view.LeaveAsync();
        Close();
    }

    private void OnClosed(object sender, WindowEventArgs args)
    {
        App.Shell?.ForgetComposerWindow(this);
        _view.Teardown();
    }
}
