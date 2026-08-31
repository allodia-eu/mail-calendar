// A drawn grid has to be taught to speak.
//
// A canvas has no accessibility tree, that is the bill that comes with the frame budget, and §4's
// "keeps its full spoken label" does not bend for it. A block too short to draw its own title still
// says its title, its time and its calendar; a "+N" chip says what it is hiding; a week the core has
// not answered for says so out loud rather than presenting itself as empty.
//
// **The nodes are placed by the renderer's OWN geometry, not by a second copy of it.** They call the
// same SurfaceMetrics.BlockRect / BandRect / MoreRect the canvas draws with, translated by the same
// two scroll offsets. A screen reader announcing an event somewhere other than where it is drawn is a
// bug that no sighted test, and no sighted developer, will ever see.
//
// Windows makes this cheaper than Android did. UIA is PULL-based: GetChildrenCore runs only when a
// client actually walks the tree, so an unobserved grid pays nothing at all, and there is no
// "is a screen reader listening?" flag to get wrong. Android had to materialize real layout nodes and
// gate them on touch exploration, because a pinch would otherwise pay for them sixty times a second.
using System;
using System.Collections.Generic;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Windows.Foundation;

namespace Allodia.Mailcal.Calendar;

/// <summary>The grid, as a screen reader sees it.</summary>
internal sealed class CalendarSurfaceAutomationPeer(CalendarSurface owner)
    : FrameworkElementAutomationPeer(owner)
{
    protected override string GetClassNameCore() => "CalendarSurface";

    protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Table;

    protected override string GetNameCore() => owner.PeriodTitle;

    /// <summary>
    /// The page in view, as spoken nodes.
    /// </summary>
    /// <remarks>
    /// Only the page <b>in view</b>. A screen reader cannot reach a week half off the side of the
    /// screen, and offering it nodes for one would let it read out a week the user is not on.
    /// </remarks>
    protected override IList<AutomationPeer> GetChildrenCore()
    {
        var children = new List<AutomationPeer>();
        owner.BuildSpokenNodes(children);
        return children;
    }
}

/// <summary>
/// One spoken thing on the grid: an event, an all-day bar, a "+N" chip, or the loading strip.
/// </summary>
/// <remarks>
/// Not backed by a <see cref="UIElement"/>, there isn't one, because the grid is drawn. It carries
/// its own name and its own rectangle, and that is all UIA needs to announce it and to put a
/// touch-exploration finger on it.
/// <para>
/// <b>The rectangle is a function, not a value</b>, and the instances are <b>cached</b> by the
/// surface. Both were learned the hard way. UIA walks the tree by asking for a node's <i>next
/// sibling</i>, which re-enters <c>GetChildrenCore</c>: mint fresh peers there and the walker cannot
/// find the node it is standing on in the new list, so it stops dead, the grid reported exactly
/// <b>one</b> spoken child out of dozens, and the events, the chips and the first all-day bar were
/// simply invisible to a screen reader. A live rect then lets the cached peers survive a scroll and a
/// zoom without being rebuilt.
/// </para>
/// </remarks>
internal sealed class CalendarItemPeer(
    string name,
    Func<GridRect> rect,
    Func<double> scale,
    FrameworkElementAutomationPeer parent,
    AutomationControlType type) : AutomationPeer
{
    protected override string GetNameCore() => name;

    protected override string GetClassNameCore() => "CalendarItem";

    protected override AutomationControlType GetAutomationControlTypeCore() => type;

    /// <summary>It is content, not chrome, a screen reader in "content" mode must still find it.</summary>
    protected override bool IsContentElementCore() => true;

    protected override bool IsControlElementCore() => true;

    protected override bool IsKeyboardFocusableCore() => false;

    protected override bool IsEnabledCore() => true;

    protected override bool IsOffscreenCore() => false;

    protected override string GetAutomationIdCore() => string.Empty;

    protected override IList<AutomationPeer> GetChildrenCore() => [];

    /// <summary>
    /// The node's rectangle, in screen coordinates.
    /// </summary>
    /// <remarks>
    /// The grid hands us a rect in its own client coordinates (DIPs); UIA wants the screen, in
    /// physical pixels. The parent's own bounding rectangle is the physical origin, and the DIP offsets
    /// are scaled to pixels by the display's rasterization factor (<see cref="GridRect.ToScreen"/>), so
    /// the node is anchored to whatever the renderer drew, at any display scale, rather than only at
    /// 100%. Anchoring to the parent (not a second guess at where the surface went) is the point.
    /// </remarks>
    protected override Rect GetBoundingRectangleCore()
    {
        var origin = parent.GetBoundingRectangle();
        var (x, y, w, h) = rect().ToScreen(origin.X, origin.Y, scale());
        return new Rect(x, y, Math.Max(w, 0d), Math.Max(h, 0d));
    }

    protected override Point GetClickablePointCore()
    {
        var r = GetBoundingRectangleCore();
        return new Point(r.X + (r.Width / 2), r.Y + (r.Height / 2));
    }

    protected override object GetPatternCore(PatternInterface patternInterface) => null!;

    protected override string GetLocalizedControlTypeCore() => string.Empty;

    protected override string GetHelpTextCore() => string.Empty;

    protected override string GetItemStatusCore() => string.Empty;

    protected override string GetItemTypeCore() => string.Empty;

    protected override string GetAcceleratorKeyCore() => string.Empty;

    protected override string GetAccessKeyCore() => string.Empty;

    protected override AutomationOrientation GetOrientationCore() => AutomationOrientation.None;

    protected override bool HasKeyboardFocusCore() => false;

    protected override bool IsPasswordCore() => false;

    protected override bool IsRequiredForFormCore() => false;

    protected override AutomationPeer? GetLabeledByCore() => null;

    protected override void SetFocusCore()
    {
        // Nothing to focus: the grid is one canvas, and the node is a description of part of it.
    }
}
