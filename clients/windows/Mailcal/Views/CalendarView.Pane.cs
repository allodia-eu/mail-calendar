// The calendar's side of the pane beside it (docs/calendar-pane.md): which days are on screen, said
// whenever they change, and the one path every choice of layout takes, the view menu's, a day in the
// month grid and a day pressed in the mini month alike (rule 5).

using System;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class CalendarView
{
    /// <summary>The first day and the count the time grid last came to rest on; <c>null</c> before it
    /// has.</summary>
    private (DateOnly First, int Count)? _rest;

    /// <summary>The layout on screen. The pager knows the grid's and the month's, but the agenda
    /// leaves it as it was.</summary>
    private CalendarMode _shown = CalendarMode.Week;

    /// <summary>Raised when the days on screen may have changed: the grid came to rest, the month
    /// paged or the layout changed. Never per frame of a scroll (rule 12).</summary>
    internal event Action? ShownDaysChanged;

    /// <summary>Where the calendar is, for the pane's pull.</summary>
    internal GridPlace Place
    {
        get
        {
            var mode = _shown;
            if (mode == CalendarMode.Month)
            {
                return new GridPlace(CalendarLayout.Month, _monthAnchor, 0);
            }
            if (!mode.IsGrid())
            {
                return new GridPlace(CalendarLayout.Agenda, Today(), 0);
            }
            var (first, count) = _rest ?? (_pager.AnchorFor(0), mode.GridColumns());
            return new GridPlace(mode.ToLayout(), first, (uint)count);
        }
    }

    /// <summary>Today in the display zone, or by this machine's clock before a model is bound.</summary>
    private DateOnly Today() => Model?.Today() ?? DateOnly.FromDateTime(DateTime.Now);

    /// <summary>A day pressed in the mini month: the seat the core chose for it, applied as the view
    /// menu applies a choice.</summary>
    internal void ApplySeat(CalendarSeat seat)
    {
        var anchor = CalendarPaint.ParseDate(seat.Anchor);
        Log.Info($"cal: seat {seat.Layout} at {anchor:yyyy-MM-dd} from the pane");
        ChooseLayout(seat.Layout.ToMode(), anchor);
    }

    /// <summary>
    /// Draws <paramref name="mode"/> and persists it, on <paramref name="at"/> when given: the month
    /// holding it, or a grid framed on it. Without one, the month opens on today's and a grid stays on
    /// the period on screen, unless it is leaving the month, which has no page to carry.
    /// </summary>
    private void ChooseLayout(CalendarMode mode, DateOnly? at)
    {
        if (Model is null)
        {
            return;
        }
        var wasMonth = IsMonth;
        Model.SetCalendarLayout(mode.ToLayout());
        ShowMode(mode);

        if (mode == CalendarMode.Month)
        {
            _pager.SetMode(CalendarMode.Month, 0);
            _monthAnchor = FirstOfMonth(at ?? Today());
            RefreshMonth();
            return;
        }

        if (!mode.IsGrid())
        {
            // The agenda: nothing to seat, and the nav cluster is already hidden.
            ShownDaysMoved();
            return;
        }

        // A shape picked from the MENU re-seats the grid on the period you are looking at and re-seeds
        // the day axis. A pinch does neither, that is the difference between SetMode and SetZoom, and
        // it is why a zoom cannot make the days jump (§2, §3).
        _pager.SetMode(mode, 0);
        if (at is { } day)
        {
            _pager.JumpTo(day);
        }
        else if (wasMonth)
        {
            _pager.JumpTo(Today());
        }
        if (Model.CalendarDisplaySettings() is { } display)
        {
            _grid.Apply(display);
        }
        _grid.Recentre(_pager.Mode, at);
    }

    /// <summary>The time grid came to rest with <paramref name="count"/> days from
    /// <paramref name="first"/> on screen.</summary>
    private void OnGridRest(DateOnly first, int count)
    {
        _rest = (first, count);
        ShownDaysMoved();
    }

    private void ShownDaysMoved() => ShownDaysChanged?.Invoke();
}
