// The two decisions behind opening a message in a window of its own (docs/reading-window.md):
// which window a message belongs in, and whether the click that just arrived was the second of a
// double-click.
//
// WinUI-free on purpose, so Mailcal.Tests can reach both. Each fails silently in the running app.
// A reader id that is not derived from the message stacks a second window on every double-click
// instead of bringing the open one forward, and the two windows then hold two copies of the same
// body. A double-click the list reads as two single clicks opens no window at all, while two
// deliberate single clicks read as a double-click opens one nobody asked for.

using System;

namespace Allodia.Mailcal.Services;

/// <summary>Which of the core's reading slots a detached window reads.</summary>
public static class ReadingWindows
{
    /// <summary>
    /// The core reader id for a message's own window.
    /// </summary>
    /// <remarks>
    /// Derived from the message rather than minted, which is what makes opening the same message
    /// twice reach the window that is already up instead of stacking a second one on it. The pair
    /// is unique: a provider key is unique within its account. The core's pane slot is a different
    /// kind of thing and no string produced here can name it (docs/reading-window.md).
    /// </remarks>
    public static string IdFor(string account, string key) => $"{account}/{key}";
}

/// <summary>
/// Tells a double-click apart from two single clicks on the same message row.
/// </summary>
/// <remarks>
/// It reads the list's own click event rather than a <c>DoubleTapped</c> handler, because the two
/// are not alternatives here: the list raises its click on the FIRST press of a double-click, and
/// by the time a separate double-tap handler ran the pane would already have opened the message
/// and the contract's "a double-click leaves the pane alone" would be lost. Deciding inside the
/// click handler is what lets the second click be recognised before anything acts on it.
/// </remarks>
public sealed class RowDoubleClick
{
    private readonly TimeSpan _interval;
    private string? _row;
    private DateTimeOffset _at;

    /// <summary>
    /// Counts two clicks on one row as a double-click when they are no more than
    /// <paramref name="interval"/> apart. The caller passes the host's own setting rather than a
    /// literal: a person who has slowed their double-click down has done so for a reason, and a
    /// hardcoded 500 ms would ignore it.
    /// </summary>
    public RowDoubleClick(TimeSpan interval) => _interval = interval;

    /// <summary>
    /// Whether the click on <paramref name="row"/> at <paramref name="now"/> completes a
    /// double-click. A different row, or too long a gap, is a first click and is remembered as one.
    /// </summary>
    /// <remarks>
    /// A completed double-click forgets what it was counting, so a third click starts again rather
    /// than completing a second double-click: a triple-click opens one window, not two.
    /// </remarks>
    public bool CompletesDoubleClick(string row, DateTimeOffset now)
    {
        if (_row == row && now - _at <= _interval)
        {
            _row = null;
            return true;
        }
        _row = row;
        _at = now;
        return false;
    }
}
