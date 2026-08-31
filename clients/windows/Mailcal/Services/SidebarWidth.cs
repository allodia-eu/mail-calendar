// What width the folder pane is allowed to be, the arithmetic behind the drag, kept apart from
// the drag itself.
//
// Its own WinUI-free file so it can be linked into Mailcal.Tests: the pointer handling needs a
// window and a real mouse (and synthetic pointer input does not reach this client's content at
// all, the shipped list|reading splitter is no more drivable than this one), so the clamp is the
// only half a test can reach. It is also the half with the interesting failure: the bounds cross
// on a narrow window, and a Math.Clamp with min > max throws.

namespace Allodia.Mailcal.Services;

/// <summary>The bounds the folder pane may be dragged between, and the clamp that applies them.</summary>
internal static class SidebarWidth
{
    /// <summary>The narrowest the pane may be dragged.</summary>
    /// <remarks>
    /// Wide enough that the longest synthetic row, "All Inboxes" with a four-figure badge beside
    /// it, still reads. Below this the pane stops being a folder tree and becomes a column of
    /// ellipses, which is the state the splitter exists to get people out of.
    /// </remarks>
    public const double Min = 200;

    /// <summary>The widest, before the window's own size has a say.</summary>
    public const double Max = 560;

    /// <summary>What must be left for the mail beside it, whatever the user drags.</summary>
    public const double MinContent = 480;

    /// <summary>The default, matching the pane's declared <c>OpenPaneLength</c>.</summary>
    public const double Default = 240;

    /// <summary>
    /// <paramref name="width"/> brought within the bounds a window of <paramref name="available"/>
    /// logical pixels allows.
    /// </summary>
    /// <remarks>
    /// Returns <see cref="Min"/> when the window cannot honour both floors at once, the bounds
    /// have crossed, and a cramped window is not a reason to take the folder tree away. Returning
    /// the floor rather than <c>Math.Clamp</c>ing between crossed bounds is not a style choice:
    /// <c>Math.Clamp(x, 200, 150)</c> throws.
    /// </remarks>
    public static double Clamp(double width, double available)
    {
        var max = Math.Min(Max, available - MinContent);
        return max < Min ? Min : Math.Clamp(width, Min, max);
    }
}
