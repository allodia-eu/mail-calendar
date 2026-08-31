// The units the grid is built out of, and deliberately nothing else.
//
// Everything in this folder that is *pure* (the zoom, the paging, the state machine, the gesture
// owner, the animation driver) compiles with no WinUI, no Win2D and no Windows SDK projection at
// all. That is not tidiness for its own sake: it is what lets the test project link these very
// source files into a plain `net10.0` assembly and run the whole contract, including the flick
// race, headlessly, with no UI framework and no test host. The moment one of them reaches for a
// `Windows.Foundation.Rect`, that stops being true.
//
// Hence this file. A rectangle, two spans, and the two constants a week is made of.
namespace Allodia.Mailcal.Calendar;

/// <summary>
/// A rectangle in the grid's own coordinates, in pixels.
/// </summary>
/// <remarks>
/// Deliberately not <c>Windows.Foundation.Rect</c>: the pure layer must stay free of the Windows
/// projection so the state machine and the gesture owner can be tested in a plain assembly. The
/// renderer converts at the boundary, which is one multiplication and no risk.
/// </remarks>
internal readonly record struct GridRect(float Left, float Top, float Right, float Bottom)
{
    /// <summary>The rectangle's width. Never negative for a rectangle the grid produced.</summary>
    internal float Width => Right - Left;

    /// <summary>The rectangle's height.</summary>
    internal float Height => Bottom - Top;

    /// <summary>The same rectangle, moved by <paramref name="dx"/> and <paramref name="dy"/>.</summary>
    /// <remarks>
    /// This is how the accessibility overlay places its nodes: the renderer's rect, translated by
    /// exactly the scroll offsets the canvas draws with. Two copies of that arithmetic would be two
    /// chances to disagree, and a screen reader announcing an event somewhere other than where it is
    /// drawn is a bug no sighted test will ever catch.
    /// </remarks>
    internal GridRect Translate(float dx, float dy) =>
        new(Left + dx, Top + dy, Right + dx, Bottom + dy);

    /// <summary>
    /// This rectangle placed into <b>screen</b> coordinates: its offsets scaled by the display's
    /// rasterization <paramref name="scale"/> and added to a physical-pixel origin
    /// (<paramref name="originX"/>, <paramref name="originY"/>).
    /// </summary>
    /// <remarks>
    /// The accessibility overlay's one job at the boundary. The grid's geometry is in DIPs (a
    /// <c>CanvasSwapChain</c> sized in DIPs at <c>96 × RasterizationScale</c> maps them to pixels for
    /// the paint), but UIA reports every bounding rectangle in <b>physical pixels</b>, so a node that
    /// added its DIP offset to the physical origin unscaled would sit right only at 100%, and on a
    /// scaled display would drift toward the surface's top-left, worse the further from it. That is
    /// exactly a screen reader announcing an event somewhere other than where it is drawn, the bug
    /// <see cref="Translate"/> and the whole spoken overlay exist to avoid, and one no sighted test on a
    /// 100% display will ever see. Kept here, pure, so <c>GridRectScreenTests</c> pins the conversion
    /// without a UI.
    /// </remarks>
    internal (double X, double Y, double Width, double Height) ToScreen(
        double originX,
        double originY,
        double scale) =>
        (originX + (Left * scale), originY + (Top * scale), Width * scale, Height * scale);

    /// <summary>Whether the point (<paramref name="x"/>, <paramref name="y"/>) falls inside.</summary>
    /// <remarks>
    /// The half-open convention (right/bottom exclusive) that adjacent day columns and stacked lanes
    /// already tile with: a point on a shared edge belongs to exactly one rectangle, so a tap can never
    /// land on two events at once.
    /// </remarks>
    internal bool Contains(float x, float y) =>
        x >= Left && x < Right && y >= Top && y < Bottom;
}

/// <summary>
/// Where one all-day bar sits: <paramref name="Days"/> columns from <paramref name="Day"/>, stacked
/// in <paramref name="Lane"/>.
/// </summary>
/// <remarks>
/// Only the geometry, so the one tested overflow count serves both the renderer (which has colours
/// and a spoken label attached) and a test (which needs neither).
/// </remarks>
internal readonly record struct BandSpan(int Day, int Days, int Lane);

/// <summary>
/// Where one timed block sits, straight from the core: a day index, two wall-clock minutes, and a
/// column fraction. Carries no pixels, that is the client's job, and the whole of it.
/// </summary>
/// <remarks>
/// Split out from the block's *paint* (its colours and its shaped labels) on purpose. The renderer
/// and the accessibility overlay both need the rectangle; only the renderer needs the colours. Kept
/// together, the rect arithmetic would drag Win2D into the pure layer and out of the tests.
/// </remarks>
internal readonly record struct BlockSpan(
    int Day,
    int Column,
    int Columns,
    int StartMinutes,
    int EndMinutes)
{
    /// <summary>How long the block runs, in minutes. The core floors this at 15.</summary>
    internal int Minutes => EndMinutes - StartMinutes;
}

/// <summary>The constants a week and a day are made of.</summary>
internal static class CalendarUnits
{
    /// <summary>
    /// The days in a page. A page is a week, the boundary a horizontal scroll cannot cross.
    /// </summary>
    internal const int DaysInWeek = 7;

    /// <summary>The hours in a day. The vertical axis spans all of them, whatever the horizon shows.</summary>
    internal const int HoursInDay = 24;

    /// <summary>The minutes in an hour, so the multiplication reads as what it is.</summary>
    internal const float MinutesInHour = 60f;
}
