// The calendar detail, the drawn time grid, plus the agenda it can fall back to.
//
// This host is deliberately thin. Everything that can be wrong about a calendar, what a gesture
// means, when a week is banked, how far the pixels may lag, what a settled pinch snaps to, what a
// "+N" chip is hiding, lives in Calendar/ and is tested headlessly in Mailcal.Tests. What is here is
// wiring: hand the grid the core's page query, and give the header somewhere to put the period title
// the grid computed.
//
// The grid is added from code-behind rather than declared in XAML because CalendarSurface is
// `internal`, and the XAML compiler wants public types in markup. Making it public to satisfy a
// markup compiler would widen the surface of a control nothing outside this assembly should touch.

using System;
using System.Globalization;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>The calendar detail view: the drawn time grid, and the agenda.</summary>
public sealed partial class CalendarView : UserControl
{
    private readonly CalendarSurface _grid = new();
    private readonly MonthGridView _month = new();

    // Over the drawn grid, not in it, so it stays on screen at any hour. Silent: the grid's spoken
    // summary already says its week is loading.
    private readonly CalendarLoadingPill _loading = new(L10n.CalendarLoadingRange(), spoken: false, "CalendarGridLoading");
    private CalendarPager _pager = new(DateOnly.FromDateTime(DateTime.Now));

    /// <summary>The calendar's focus date, the day every view is drawn around (docs/calendar.md, "The
    /// focus date"); <c>null</c> until <see cref="Init"/> binds the core.</summary>
    private CalendarFocus? _focus;

    /// <summary>The month the month grid is showing, the 1st of it. The month is not the swipe-paged
    /// time grid, so it carries its own anchor rather than riding the pager's page counter.</summary>
    private DateOnly _monthAnchor = new(DateTime.Now.Year, DateTime.Now.Month, 1);

    /// <summary>The display settings the grid was last seeded with, so a settings signal can tell what
    /// actually changed, a week-start change re-seats and re-frames; the rest only re-apply.</summary>
    private DisplaySettings? _appliedDisplay;

    /// <summary>
    /// Whether the grid has been seated from the core yet, its week aligned, its shape and horizon
    /// applied.
    /// </summary>
    /// <remarks>
    /// This is the fix for a boot-order race. The calendar can be <b>shown before the account
    /// connects</b> (the <c>--calendar</c> flag, the Start-menu tile): at that point the core has no
    /// week-start to give, so <c>week_start_date</c> falls back to the identity and the pager would
    /// seed on today (a Tuesday under the first heading) instead of the week's first day. And the
    /// week start may never be derived client-side (§3), so the seat must simply <i>wait</i> for the
    /// core, the first calendar or settings signal after connect seats it, aligned, exactly once.
    /// </remarks>
    private bool _seatedFromCore;

    /// <summary>The shared app model (set by the host via <see cref="Init"/>).</summary>
    public MailboxModel? Model { get; private set; }

    /// <summary>Initialises the control.</summary>
    public CalendarView()
    {
        this.InitializeComponent();
        GridHost.Children.Add(_grid);
        GridHost.Children.Add(_loading);
        MonthHost.Children.Add(_month);

        // The automation id goes on the SURFACE, not on the host it sits in: a bare layout Grid
        // gets no automation peer, so an id on one reaches nothing and a test waiting for it can
        // only time out (clients/windows/uia.ps1, "what UIA cannot reach"). Set here for the same
        // reason the surface is added here, the type may not appear in markup.
        //
        // The month surface gets none, and that is not an oversight: it overrides no
        // OnCreateAutomationPeer, so it has no peer to carry one, and an id on it would reach
        // nothing exactly as the host Grid did. That it is absent from the automation tree is the
        // visible half of a real gap, a screen reader cannot read the month at all
        // (docs/calendar.md, "Known gaps").
        AutomationProperties.SetAutomationId(_grid, "CalendarGrid");

        _grid.PeriodChanged = () => PeriodText.Text = _grid.PeriodTitle;
        _grid.LoadingPillChanged = () =>
        {
            var place = _grid.LoadingPill;
            _loading.Margin = new Thickness(place.Left, place.Top, 0, 0);
            _loading.Show(place.Shown);
        };
        _grid.ZoomSettled = OnZoomSettled;
        _grid.OpenEvent = OnOpenEvent;
        _grid.NewEventHex = NewEventHex;
        _grid.CreateAt = OnCreateAt;
        _grid.MoveDropped = OnMoveDropped;
        _month.DayPicked = OnMonthDayPicked;
        _month.EventPicked = OnOpenEvent;
    }

    private bool IsMonth => _pager.Mode == CalendarMode.Month;

    private static DateOnly FirstOfMonth(DateOnly d) => new(d.Year, d.Month, 1);

    /// <summary>Binds the view to the shared model.</summary>
    public void Init(MailboxModel model)
    {
        Model = model;
        this.Bindings.Update();

        // A launch opens on today: the focus is not persisted.
        _focus = new CalendarFocus(new CoreCalendarFocus(model), model.Today());
        _focus.Set(model.Today());

        // The grid's page query. A pull with an argument, so the CLIENT owns the anchor and the core
        // never learns where the user is, which is what makes it impossible for two quick swipes to
        // race and settle the grid on last week (docs/calendar.md §5).
        _grid.PageFor = (from, columns) => model.CalendarRange(from, columns) ?? EmptyPage(from);
        _grid.AnchorFor = week => _pager.AnchorFor(week);
        _grid.WeekStartFor = model.WeekStart;
        _grid.Today = model.Today;
        _grid.NowMinutes = () => model.WallClock() is var now ? (now.Hour * 60) + now.Minute : 0;
        _grid.CameToRest = OnGridRest;

        model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MailboxModel.CalendarVersion))
            {
                if (!_seatedFromCore)
                {
                    // Shown before the account connected; the core can answer now, so seat aligned.
                    SeatFromCore();
                }
                else
                {
                    // "Calendar data changed, re-pull whatever you are showing." Not a snapshot.
                    _grid.Rebuild();
                    if (IsMonth)
                    {
                        RefreshMonth();
                    }
                }
            }
            else if (e.PropertyName == nameof(MailboxModel.DisplaySettingsVersion))
            {
                // A display setting changed (week start, clock, horizon), re-apply it to the grid.
                ReapplyDisplaySettings();
            }
        };

        // May be a no-op if the core has not connected yet, a signal above seats it once it has.
        SeatFromCore();
    }

    /// <summary>
    /// Called when the calendar is brought on screen, so it opens on today.
    /// </summary>
    /// <remarks>
    /// It must open on <b>today</b>, scrolled to <b>now</b>, not on column 0 at midnight. On a Sunday
    /// with a Monday-start week, today is the <i>last</i> column, and opening on column 0 shows a week
    /// that does not visibly contain today.
    /// </remarks>
    internal void OnShown()
    {
        if (Model is null)
        {
            return;
        }
        // Re-seat on today whenever the calendar is brought on screen, but only if the core is ready.
        // On a cold --calendar boot it is not, so this is a no-op and the first signal after connect
        // seats it (the CalendarVersion handler in Init).
        _seatedFromCore = false;
        SeatFromCore();
    }

    /// <summary>
    /// Seats the grid from the core: applies the persisted shape, horizon and clock, aligns the week,
    /// and frames it.
    /// </summary>
    /// <remarks>
    /// Requires the core to be connected. If it is not, a cold <c>--calendar</c> boot, before the
    /// account is up, this is a no-op and leaves <see cref="_seatedFromCore"/> <c>false</c>, so the
    /// first calendar/settings signal after connect retries it. The week start can only come from the
    /// core (§3), so this genuinely has to wait rather than guess.
    /// </remarks>
    private void SeatFromCore()
    {
        if (Model?.CalendarDisplaySettings() is not { } display)
        {
            return;
        }

        ApplyDisplaySettings(display);

        // Every seat builds the pager with the core's week start, the agenda and the month included:
        // they align nothing themselves, but the week a later menu choice or Today seats is aligned
        // only by the pager they leave behind.
        var mode = display.Layout.ToMode();
        _pager = new CalendarPager(
            Today(),
            mode.IsGrid() ? mode : _pager.Mode,
            Model.WeekStart);
        if (mode.IsGrid())
        {
            _grid.Recentre(mode);
        }

        _seatedFromCore = true;
        Log.Info($"cal: seated mode={mode} weekStart={display.WeekStart} " +
            $"hours={display.VisibleHours} clock={display.TimeFormat}");
    }

    private void ApplyDisplaySettings(DisplaySettings display)
    {
        // The shape and the horizon are the CORE's, so the calendar reopens the way it was left, and
        // so the desktop and the phone open on the same one (§8).
        var mode = display.Layout.ToMode();
        if (mode.IsGrid())
        {
            _pager.SetZoom(mode);
        }
        else if (mode == CalendarMode.Month)
        {
            _pager.SetMode(CalendarMode.Month, 0);
        }
        ShowMode(mode);
        _grid.Apply(display);
        if (mode == CalendarMode.Month)
        {
            _monthAnchor = FirstOfMonth(_focus?.Focus ?? Today());
            RefreshMonth();
        }
        _appliedDisplay = display;
    }

    /// <summary>
    /// Re-applies the display settings after the core signals a change (week start, clock, horizon).
    /// </summary>
    /// <remarks>
    /// The horizon and clock re-apply cheaply, <see cref="ApplyDisplaySettings"/> re-seeds the hours
    /// and re-pulls. But a <b>week-start</b> change moves the grid's alignment, so it also re-seats and
    /// re-frames the pager. That re-seat is gated on the week-start actually changing: a Settings signal
    /// fires for <i>any</i> setting (a swipe action, a send account), and re-seating on all of them
    /// would jerk the calendar back to today whenever the user touched an unrelated preference.
    /// </remarks>
    private void ReapplyDisplaySettings()
    {
        if (Model?.CalendarDisplaySettings() is not { } display)
        {
            return;
        }
        if (!_seatedFromCore)
        {
            // The first settings signal after a pre-connect show, seat aligned rather than diff
            // against a state that was never applied.
            SeatFromCore();
            return;
        }
        var weekStartChanged = _appliedDisplay is null || _appliedDisplay.WeekStart != display.WeekStart;
        Log.Info($"cal: display settings applied weekStart={display.WeekStart} " +
            $"hours={display.VisibleHours} clock={display.TimeFormat} reseat={weekStartChanged}");
        ApplyDisplaySettings(display);
        if (weekStartChanged)
        {
            // The week moves under the focus, which stays where it is.
            var focus = _focus?.Focus ?? Today();
            _pager = new CalendarPager(Today(), _pager.Mode, Model.WeekStart);
            _pager.JumpTo(focus);
            _grid.Recentre(_pager.Mode, focus);
        }
    }

    /// <summary>
    /// A settled pinch: persist the shape and the horizon it landed on.
    /// </summary>
    /// <remarks>
    /// Only on <b>lift</b>, a save per frame would push a preference write across the FFI dozens of
    /// times a second. And it <i>zooms</i> the pager rather than re-seating it: a zoom must leave the
    /// week exactly where it is (§3).
    /// </remarks>
    private void OnZoomSettled(CalendarMode mode, int hours)
    {
        _pager.SetZoom(mode);
        Model?.SetCalendarVisibleHours((byte)hours);
        Model?.SetCalendarLayout(mode.ToLayout());
    }

    private void OnPickView(object sender, RoutedEventArgs e)
    {
        if (Model is null || (sender as FrameworkElement)?.Tag is not string tag)
        {
            return;
        }

        var mode = tag switch
        {
            "day" => CalendarMode.Day,
            "three" => CalendarMode.ThreeDay,
            "work" => CalendarMode.WorkWeek,
            "week" => CalendarMode.Week,
            "month" => CalendarMode.Month,
            _ => CalendarMode.Agenda,
        };

        ChooseLayout(mode, at: null);
    }

    // Shows the one surface this shape needs, and names the chevrons for what they will now do.
    private void ShowMode(CalendarMode mode)
    {
        _shown = mode;
        var agenda = mode == CalendarMode.Agenda;
        var month = mode == CalendarMode.Month;
        var grid = !agenda && !month;
        GridHost.Visibility = grid ? Visibility.Visible : Visibility.Collapsed;
        AgendaList.Visibility = agenda ? Visibility.Visible : Visibility.Collapsed;
        MonthHost.Visibility = month ? Visibility.Visible : Visibility.Collapsed;

        // The agenda is one forward-running list that always contains today: there is no previous, no
        // next, and nothing for "today" to jump to. Everywhere else the cluster is always there, its
        // whole point is that it does not move (see the XAML).
        NavCluster.Visibility = agenda ? Visibility.Collapsed : Visibility.Visible;
        if (agenda)
        {
            return;
        }

        // The month titles the header itself, and the grid raises PeriodChanged only when its own title
        // changes, so a grid returning to the days it last showed would leave the month's name over them.
        if (grid && _grid.PeriodTitle.Length > 0)
        {
            PeriodText.Text = _grid.PeriodTitle;
        }

        // Spoken as what it does *here*: "next day" in the day zoom, "next 3 days", "next week", "next
        // month". A screen-reader user gets told how far the button will move them, which is the only
        // thing about it worth knowing.
        var (prev, next) = mode switch
        {
            CalendarMode.Month => (L10n.CalendarPrevMonth(), L10n.CalendarNextMonth()),
            CalendarMode.Day => (L10n.CalendarPrevDay(), L10n.CalendarNextDay()),
            CalendarMode.ThreeDay => (L10n.CalendarPrevDays(3), L10n.CalendarNextDays(3)),
            _ => (L10n.CalendarPrevWeek(), L10n.CalendarNextWeek()),
        };
        AutomationProperties.SetName(PrevButton, prev);
        AutomationProperties.SetName(NextButton, next);
        ToolTipService.SetToolTip(PrevButton, prev);
        ToolTipService.SetToolTip(NextButton, next);
    }

    // Pulls the anchored month and hands it to the grid, and titles the header from it.
    private void RefreshMonth()
    {
        if (Model?.MonthPage(_monthAnchor) is not { } page)
        {
            return;
        }
        var display = Model.CalendarDisplaySettings();
        var use24 = display is null || display.TimeFormat == TimeFormat.TwentyFourHour;
        _month.Apply(page, Today(), CultureInfo.CurrentCulture, use24);
        PeriodText.Text = _monthAnchor.ToString("MMMM yyyy", CultureInfo.CurrentCulture);

        var chips = 0;
        foreach (var cell in page.Cells)
        {
            chips += cell.Chips.Length;
        }
        Log.Info($"cal: month {_monthAnchor:yyyy-MM} cells={page.Cells.Length} chips={chips} " +
            $"materialized={page.IsMaterialized}");
        ShownDaysMoved();
    }

    private void OnPrevPeriod(object sender, RoutedEventArgs e) => Step(-1);

    private void OnNextPeriod(object sender, RoutedEventArgs e) => Step(1);

    /// <summary>One chevron click: the core moves the focus one period of the view on screen, and the
    /// view is drawn around where it landed.</summary>
    /// <remarks>
    /// The grid's step is an <b>animated scroll of the same strip a swipe moves</b>, to the first day of
    /// the period the core seats the focus in, so it can be scrolled straight back and a burst of presses
    /// lands on the last one. How far a step goes is the core's, a month on the same day of the month
    /// included; nothing here counts days.
    /// </remarks>
    private void Step(int direction)
    {
        var mode = IsMonth ? CalendarMode.Month : _pager.Mode;
        if (Model is null || _focus is null || !(mode.IsGrid() || mode.IsMonth()))
        {
            return;
        }
        var focus = _focus.Step(mode, direction, Today());
        if (mode.IsMonth())
        {
            _monthAnchor = FirstOfMonth(focus);
            RefreshMonth();
            return;
        }
        if (Model.CalendarSeat(focus, mode.ToLayout()) is { } seat)
        {
            _grid.SlideTo(CalendarPaint.ParseDate(seat.Anchor));
        }
    }

    // A press on a month day's own space: the focus moves there and the month stays on screen.
    private void OnMonthDayPicked(DateOnly date)
    {
        if (_focus is null)
        {
            return;
        }
        Log.Info("cal: focus a day in the month");
        _focus.Set(date);
        RefreshMonth();
    }

    private void OnBackToToday(object sender, RoutedEventArgs e)
    {
        var today = Today();
        Log.Info($"cal: back to today (mode={_pager.Mode})");
        _focus?.Set(today);
        if (IsMonth)
        {
            _monthAnchor = FirstOfMonth(today);
            RefreshMonth();
            return;
        }
        _pager.JumpTo(today);
        _grid.Recentre(_pager.Mode);
    }

    /// <summary>
    /// A week the core has not answered for.
    /// </summary>
    /// <remarks>
    /// <c>isMaterialized: false</c>, deliberately, it means "we have not looked", not "no events",
    /// and the grid must say so rather than draw a confidently empty week (§4).
    /// </remarks>
    private static CalendarPage EmptyPage(DateOnly from)
    {
        var days = new GridDay[CalendarUnits.DaysInWeek];
        for (var i = 0; i < days.Length; i++)
        {
            var day = from.AddDays(i);
            // Week 0: the core numbers weeks, and it has not answered for this one.
            days[i] = new GridDay(day.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture), 0);
        }
        return new CalendarPage(days, [], [], 0, "UTC", [], false);
    }

    private void OnRefresh(object sender, RoutedEventArgs e) => Model?.ShowCalendar();
}
