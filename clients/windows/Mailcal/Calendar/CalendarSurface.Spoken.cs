// The grid, spoken.
//
// Split from CalendarSurface.Draw.cs to stay under the repo's 500-line file cap (AGENTS.md); it is the
// same class. The seam is a real one: that file decides what the frame *shows*, this one decides what a
// screen reader *hears*, and §4 of the calendar contract says those must be the same grid. A drawn grid
// has no accessibility tree of its own, that is a bill, not an excuse, so every rect here is computed
// by the very functions the canvas draws with (SurfaceMetrics.BlockRect / BandRect / MoreRect), and
// there is no second copy of the arithmetic to disagree with the first.
using System.Collections.Generic;
using System.Linq;
using Microsoft.UI.Xaml.Automation.Peers;

namespace Allodia.Mailcal.Calendar;

internal sealed partial class CalendarSurface
{
    /// <summary>
    /// The event a tap at (<paramref name="x"/>, <paramref name="y"/>) falls on, an all-day band in
    /// the banner, or a timed block in the grid, as its <c>(account, event)</c>, or <c>null</c> for
    /// empty space.
    /// </summary>
    /// <remarks>
    /// Mirrors <see cref="BuildSpokenFor"/>'s geometry <b>exactly</b> (§7): a finger and a screen
    /// reader must both agree with the pixels, so both place from the renderer's own
    /// <see cref="SurfaceMetrics.BlockRect"/> / <see cref="SurfaceMetrics.BandRect"/>, translated by
    /// the same per-week strip offset. <b>Both</b> weeks are checked when the strip straddles a seam,
    /// a tap on the right-hand half of the glass is on the next week, and hit-testing only the anchor
    /// would ignore it. The banner and grid regions are split by <paramref name="y"/> so a block
    /// scrolled up under the banner cannot steal a banner tap.
    /// </remarks>
    internal EventOpen? EventAt(float x, float y)
    {
        var m = _state.Metrics(Viewport());
        var weeks = _state.SecondWeekVisible(m)
            ? new[] { _state.Week, _state.Week + 1 }
            : new[] { _state.Week };

        var inBanner = y >= m.Viewport.HeaderHeight && y < m.ContentTop;
        foreach (var week in weeks)
        {
            var page = Page(week);
            if (page.Headings.Count == 0)
            {
                continue;
            }
            var dx = m.Gutter + LeftOf(week, m);

            if (inBanner)
            {
                var drawn = CalendarAllDay.DrawnLanes(page.Lanes, _state.BannerExpanded);
                var spans = page.Bands.Select(b => b.Span).ToList();
                var i = CalendarHitTest.BandAt(spans, drawn, m, dx, x, y);
                if (i >= 0)
                {
                    var band = page.Bands[i];
                    return new EventOpen(band.Account, band.Event, band.OccurrenceStart);
                }
            }
            else if (y >= m.ContentTop)
            {
                var spans = page.Blocks.Select(b => b.Span).ToList();
                var i = CalendarHitTest.BlockAt(spans, m, dx, _state.ScrollY, x, y);
                if (i >= 0)
                {
                    var block = page.Blocks[i];
                    return new EventOpen(block.Account, block.Event, block.OccurrenceStart);
                }
            }
        }
        return null;
    }

    /// <summary>
    /// The weeks in view, as spoken nodes, placed by the renderer's own geometry.
    /// </summary>
    /// <remarks>
    /// Called by <see cref="CalendarSurfaceAutomationPeer"/>, and so only when a UIA client actually
    /// walks the tree. An unobserved grid pays nothing.
    /// <para>
    /// <b>Both</b> weeks, when the strip is straddling a seam. The grid can now come to rest showing
    /// half of each of two weeks, and a screen reader that was only ever told about the anchor week
    /// would silently omit everything on the right-hand half of the glass, which is not a rendering
    /// bug anybody would ever see (§4).
    /// </para>
    /// <para>
    /// Every rect comes from <see cref="SurfaceMetrics.BlockRect"/> / <see cref="SurfaceMetrics.BandRect"/>
    /// / <see cref="SurfaceMetrics.MoreRect"/>, the same functions the canvas draws with, translated
    /// by the same offsets. There is no second copy of the arithmetic, and so no way for a screen reader
    /// to announce an event somewhere other than where it is drawn.
    /// </para>
    /// </remarks>
    internal void BuildSpokenNodes(IList<AutomationPeer> into)
    {
        if (FrameworkElementAutomationPeer.FromElement(this) is not FrameworkElementAutomationPeer peer)
        {
            return;
        }

        var m = _state.Metrics(Viewport());
        var seam = _state.SecondWeekVisible(m);
        var anchor = Page(_state.Week);
        var next = seam ? Page(_state.Week + 1) : PagePaint.Empty;
        var key = (_state.Week, seam, _state.BannerExpanded, anchor, next);

        // **The instances must be STABLE.** UIA walks the tree by asking a node for its next sibling,
        // which re-enters GetChildrenCore, and if that hands back brand-new peers, the walker cannot
        // find the node it is standing on in the new list and stops dead. Measured: the grid reported
        // exactly ONE spoken child out of dozens, and every event was invisible to a screen reader.
        if (_spokenKey != key)
        {
            _spokenKey = key;
            _spoken.Clear();
            BuildSpokenFor(anchor, _state.Week, peer, _spoken);
            if (seam)
            {
                BuildSpokenFor(next, _state.Week + 1, peer, _spoken);
            }
        }

        foreach (var node in _spoken)
        {
            into.Add(node);
        }
    }

    /// <summary>
    /// One week's spoken nodes, each with a <b>live</b> rectangle.
    /// </summary>
    /// <remarks>
    /// Live, because the rect must follow the scroll and the zoom, and because if it did not, the
    /// list would have to be rebuilt on every frame of a pinch, which is exactly the identity churn
    /// the caching above exists to stop. <paramref name="week"/> is the absolute week index, so a rect
    /// stays correct across a re-anchor: the strip's own arithmetic (<see cref="LeftOf"/>) does the rest.
    /// </remarks>
    private void BuildSpokenFor(
        PagePaint page,
        int week,
        FrameworkElementAutomationPeer peer,
        List<AutomationPeer> into)
    {
        if (page.Headings.Count == 0)
        {
            return;
        }

        // The two translations the canvas uses, computed fresh each time a rect is asked for.
        GridRect Banner(GridRect r)
        {
            var m = _state.Metrics(Viewport());
            return r.Translate(m.Gutter + LeftOf(week, m), m.Viewport.HeaderHeight);
        }

        GridRect Grid(GridRect r)
        {
            var m = _state.Metrics(Viewport());
            return r.Translate(m.Gutter + LeftOf(week, m), m.ContentTop - _state.ScrollY);
        }

        // Every node's DIP rect becomes a physical-pixel screen rect through this, read live, so it
        // is right after a move to a differently-scaled monitor (see CalendarItemPeer / GridRect.ToScreen).
        double Scale() => DisplayScale;

        if (!page.IsMaterialized)
        {
            // A week the core has not answered for says so out loud. Rendering it as empty would be a
            // lie that looks exactly like a real answer (§4).
            into.Add(new CalendarItemPeer(
                _strings.Loading,
                () =>
                {
                    var m = _state.Metrics(Viewport());
                    var left = m.Gutter + LeftOf(week, m);
                    return new GridRect(left, m.ContentTop, left + m.WeekWidth, m.ContentTop + m.Viewport.LaneHeight);
                },
                Scale,
                peer,
                AutomationControlType.Text));
            return;
        }

        // The column headings. Without these a screen reader can hear every event on the grid and
        // still have no idea which DAY any of them is on, which makes the whole surface useless to
        // it, however faithfully the blocks are announced.
        for (var i = 0; i < page.Headings.Count; i++)
        {
            var heading = page.Headings[i];
            var column = i;
            into.Add(new CalendarItemPeer(
                $"{heading.Weekday} {heading.DayOfMonth}",
                () =>
                {
                    var m = _state.Metrics(Viewport());
                    var left = m.Gutter + LeftOf(week, m) + (m.DayWidth * column);
                    return new GridRect(left, 0f, left + m.DayWidth, m.Viewport.HeaderHeight);
                },
                Scale,
                peer,
                AutomationControlType.HeaderItem));
        }

        var drawn = CalendarAllDay.DrawnLanes(page.Lanes, _state.BannerExpanded);

        foreach (var band in page.Bands)
        {
            if (band.Span.Lane >= drawn)
            {
                continue;
            }
            var span = band.Span;
            into.Add(new CalendarItemPeer(
                band.Spoken,
                () => Banner(_state.Metrics(Viewport()).BandRect(span)),
                Scale,
                peer,
                AutomationControlType.Text));
        }

        if (drawn < page.Lanes)
        {
            for (var day = 0; day < page.MoreSpoken.Count; day++)
            {
                var spoken = page.MoreSpoken[day];
                if (spoken.Length == 0)
                {
                    continue;
                }
                var column = day;
                into.Add(new CalendarItemPeer(
                    spoken,
                    () => Banner(_state.Metrics(Viewport()).MoreRect(column, drawn)),
                    Scale,
                    peer,
                    AutomationControlType.Button));
            }
        }

        // Every block speaks, including the ones too short to DRAW their own title (§4).
        foreach (var block in page.Blocks)
        {
            var span = block.Span;
            into.Add(new CalendarItemPeer(
                block.Spoken,
                () => Grid(_state.Metrics(Viewport()).BlockRect(span)),
                Scale,
                peer,
                AutomationControlType.Text));
        }
    }
}
