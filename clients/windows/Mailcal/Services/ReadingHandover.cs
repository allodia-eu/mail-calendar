// What the reading pane shows between two messages, the rule behind "the pane changes once".
//
// Opening a message records its header immediately; its body lands a moment later, once the core
// has fetched and sanitised it. Drawing that gap tears the pane down to a spinner and rebuilds it:
// the recipient rows and the remote-images bar collapse and come back, so the toolbar jumps, and
// the message canvas blinks to the pane's own background in between. Holding what is already
// rendered for a short grace window instead means the common case, the body arrives inside it,
// changes the pane exactly once, with header, recipients, bar and body moving together. A fetch
// that outruns the window still falls back to the loading state, so nothing waits behind a message
// that is no longer the one being read.
//
// **Pure BCL on purpose, no WinUI, no view models.** Same reason as ReadingAdvance and
// ReadingSelection: Mailcal.Tests links this file and gates the rule on every PR.

using System;

namespace Allodia.Mailcal.Services;

/// <summary>What the reading pane should draw while an opened message's body is still in flight.</summary>
public enum HandoverStep
{
    /// <summary>Tear down to the loading state, there is nothing worth holding on screen.</summary>
    Loading,

    /// <summary>Keep the rendered message, and start the grace window that bounds the hold.</summary>
    StartGrace,

    /// <summary>Keep the rendered message; the grace window is already running.</summary>
    Hold,
}

/// <summary>
/// Tracks which message the reading pane has actually rendered, and decides whether to keep it on
/// screen while the next one's body is fetched.
/// </summary>
public sealed class ReadingHandover
{
    /// <summary>
    /// How long a rendered message may stand in for the one being opened. Long enough to cover a
    /// body that is already cached or on a fast connection, short enough that a slower fetch
    /// reaches its spinner while the click still feels answered.
    /// </summary>
    public static readonly TimeSpan Grace = TimeSpan.FromMilliseconds(300);

    private string? _rendered;
    private string? _waiting;
    private bool _spent;

    /// <summary>The body for <paramref name="key"/> is now on screen.</summary>
    public void Rendered(string key)
    {
        _rendered = key;
        _waiting = null;
        _spent = false;
    }

    /// <summary>The pane is showing no message body (its placeholder, or the loading state).</summary>
    public void Cleared()
    {
        _rendered = null;
        _waiting = null;
        _spent = false;
    }

    /// <summary>The grace window elapsed with the awaited body still missing.</summary>
    public void GraceElapsed() => _spent = true;

    /// <summary>
    /// What to draw while the body for <paramref name="key"/> is still in flight. The first ask for
    /// a newly opened message starts its grace window; every ask after that holds until the window
    /// is <see cref="GraceElapsed">spent</see>.
    /// </summary>
    public HandoverStep Next(string key)
    {
        // Nothing rendered to stand in, the first message of a session, a pane that has already
        // fallen back to the spinner, or a body that failed to fetch (the retry button has to
        // answer at once, so a failed message is never held). Hold nothing.
        if (_rendered is null)
        {
            return HandoverStep.Loading;
        }
        if (_waiting != key)
        {
            _waiting = key;
            _spent = false;
            return HandoverStep.StartGrace;
        }
        return _spent ? HandoverStep.Loading : HandoverStep.Hold;
    }
}
