// Where a stepped sheet stands (docs/ai.md, "Learning"): which of its pages is on screen, and which
// way the last move went, which is the side the next page arrives from. The learn sheet and the
// reveal share it. WinUI-free so Mailcal.Tests can pin it.

using System;

namespace Allodia.Mailcal.Services;

/// <summary>The page a stepped sheet shows, held to the pages there are.</summary>
internal sealed class WizardPager
{
    /// <summary>A pager over <paramref name="count"/> pages, never fewer than one, on the first.</summary>
    internal WizardPager(int count) => Count = Math.Max(count, 1);

    /// <summary>How many pages there are.</summary>
    internal int Count { get; }

    /// <summary>The page on screen.</summary>
    internal int Index { get; private set; }

    /// <summary>Whether the last move went forward; the page it reached arrives from that side.</summary>
    internal bool Forward { get; private set; } = true;

    /// <summary>Whether the first page is on screen.</summary>
    internal bool IsFirst => Index == 0;

    /// <summary>Whether the last page is on screen.</summary>
    internal bool IsLast => Index == Count - 1;

    /// <summary>
    /// Moves to <paramref name="target"/>, held to the pages there are. Returns whether the page
    /// changed; standing still is not a move, so it leaves the direction as it was.
    /// </summary>
    internal bool Go(int target)
    {
        var next = Math.Clamp(target, 0, Count - 1);
        if (next == Index)
        {
            return false;
        }
        Forward = next > Index;
        Index = next;
        return true;
    }
}
