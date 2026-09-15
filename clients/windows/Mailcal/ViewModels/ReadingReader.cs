// Where a reading view takes its message and its body from (docs/reading-window.md).
//
// The core keeps one reading slot per reader: the pane's, and one per detached window. Two views
// can be on screen at once and each must read only its own, so the view is handed a reader rather
// than reaching into the model for `OpenedMessage` and `Reading`. That is the whole of what a
// window changes about the reading view; everything else it draws is the pane's.
//
// The pane's slot is a different variant in the core and not a reserved string, so nothing a
// window mints can reach it. The same split is kept here: PaneReader reads the model's own two
// properties, WindowReader holds its own pair, and neither can be mistaken for the other.

using System;
using System.ComponentModel;
using Allodia.Mailcal.Services;

namespace Allodia.Mailcal.ViewModels;

/// <summary>Which half of a reader changed, so a view can redraw only what moved.</summary>
public enum ReadingChange
{
    /// <summary>A different message is being read: per-message view state resets.</summary>
    Opened,

    /// <summary>The body for the message being read has arrived, or been replaced.</summary>
    Body,
}

/// <summary>One viewer of a message: the row it was opened on, and the body the core holds for it.</summary>
public abstract class ReadingReader
{
    /// <summary>The list row this reader was opened on, or <c>null</c> when it is showing nothing.</summary>
    public abstract OpenedMessage? Opened { get; }

    /// <summary>The fetched body, or <c>null</c> until the open for <see cref="Opened"/> lands.</summary>
    public abstract ReadingBody? Body { get; }

    /// <summary>Raised when either half changes.</summary>
    public event EventHandler<ReadingChange>? Changed;

    /// <summary>Announces a change to whichever view is drawing this reader.</summary>
    protected void RaiseChanged(ReadingChange what) => Changed?.Invoke(this, what);
}

/// <summary>
/// The reading pane's reader: the model's own open message and body, on the shape above.
/// </summary>
/// <remarks>
/// A pass-through rather than a second copy. The model's two properties stay exactly where they
/// were, because the list, the selection and the composer's quote seed all read them, and a pane
/// whose state lived somewhere else would be a second answer to "what is open".
/// </remarks>
public sealed class PaneReader : ReadingReader
{
    private readonly MailboxModel _model;

    /// <summary>Follows <paramref name="model"/>'s pane slot for as long as the model lives.</summary>
    public PaneReader(MailboxModel model)
    {
        _model = model;
        _model.PropertyChanged += OnModelChanged;
    }

    /// <inheritdoc />
    public override OpenedMessage? Opened => _model.OpenedMessage;

    /// <inheritdoc />
    public override ReadingBody? Body => _model.Reading;

    private void OnModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(MailboxModel.OpenedMessage))
        {
            RaiseChanged(ReadingChange.Opened);
        }
        else if (e.PropertyName == nameof(MailboxModel.Reading))
        {
            RaiseChanged(ReadingChange.Body);
        }
    }
}

/// <summary>
/// One detached reading window's reader: the row the window was opened on, and the body the core
/// publishes into that window's own slot.
/// </summary>
/// <remarks>
/// The header travels with the window rather than being read back out of the body, so the window
/// draws a subject and a sender from its first frame, exactly as the pane does. A window that
/// opened on a blank header would read as broken for as long as the fetch takes.
/// </remarks>
public sealed class WindowReader : ReadingReader
{
    private ReadingBody? _body;

    /// <summary>Opens a reader on <paramref name="opened"/>, before its body exists.</summary>
    public WindowReader(OpenedMessage opened)
    {
        Opened = opened;
        Id = ReadingWindows.IdFor(opened.Account, opened.Key);
    }

    /// <summary>The core reader id this window's body arrives in.</summary>
    public string Id { get; }

    /// <inheritdoc />
    public override OpenedMessage? Opened { get; }

    /// <inheritdoc />
    public override ReadingBody? Body => _body;

    /// <summary>
    /// Takes the body the core is holding for this window.
    /// </summary>
    /// <remarks>
    /// A <c>Surface.Reading</c> signal says that <em>some</em> reader's body changed, not which, so
    /// every open window re-pulls its own and most of those pulls carry the body it already had.
    /// Re-announcing one is cheap: the view's own render guard compares the sanitised fragment and
    /// reloads the WebView2 only when it actually differs.
    /// </remarks>
    public void TakeBody(ReadingBody? body)
    {
        _body = body;
        RaiseChanged(ReadingChange.Body);
    }
}
