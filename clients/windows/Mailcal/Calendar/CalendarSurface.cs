// The time grid: one canvas, one pointer owner, five live weeks.
//
// This is the thin WinUI shell around a state machine that knows nothing about WinUI. Everything that
// can be wrong, pan versus page versus zoom, when a week is banked, how far the pixels may lag, what
// a settled pinch snaps to, lives in CalendarSurfaceState / CalendarGestureOwner / CalendarSurfaceDriver
// and is tested headlessly in Mailcal.Tests. What is left here is the translation:
//
//   PointerPressed/Moved/Released/CaptureLost  ->  CalendarGestureOwner
//   PointerWheelChanged (mouse + touchpad)     ->  CalendarGestureOwner  (the SAME owner: §6)
//   CompositionTarget.Rendering                ->  CalendarSurfaceDriver.Tick
//   CanvasControl.Draw                         ->  CalendarSurfaceDraw
//   AutomationPeer                             ->  the spoken grid
//
// **There is no ScrollViewer here, and there must never be one.** A pager plus a scroller plus a
// pinch recogniser, each reading the same finger and none able to see the others, is what produced
// both the swipe that stuck between two weeks and the zoom that panned the grid, and neither was
// fixable from inside it (§6). On Windows the trap wears a different hat: the precision touchpad's
// two-finger pan arrives as a WHEEL, not as touch, so a ScrollViewer would quietly take it. It comes
// to the owner instead.
using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Services;
using Microsoft.Graphics.Canvas;
using Microsoft.Graphics.Canvas.UI.Xaml;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>The drawn time grid, day, 3-day, work-week and week, as zoom levels of one surface.</summary>
internal sealed partial class CalendarSurface : UserControl
{
    /// <summary>
    /// How many weeks either side of the anchor are painted <b>ahead</b>, while the grid is at rest.
    /// </summary>
    /// <remarks>
    /// Far wider than the two weeks the strip can actually <i>show</i>, and deliberately so (§7).
    /// Building a not-yet-seen week's paint, parsing every event's colour, formatting its clock,
    /// assembling its spoken label, costs tens of milliseconds on a busy diary, and doing it
    /// <i>inside a fling</i> is the one hitch the frame budget still showed. So the moment the grid
    /// settles, the pages out to here are painted on idle frames (no present), and
    /// <see cref="TrimPages"/> keeps them: a fling then scrolls through weeks that are already
    /// drawable. A sustained fling past the halo still builds at its edge, bounded, and named in
    /// docs/calendar.md "Known gaps".
    /// </remarks>
    private const int PrebuildHalo = 4;

    /// <summary>How far a finger must travel before it has decided what it is.</summary>
    private const float TouchSlop = 8f;

    /// <summary>
    /// The grid presents into a swapchain <b>we own</b>, not into a composition surface.
    /// </summary>
    /// <remarks>
    /// This is not a rendering preference, it is what makes §7 checkable. A Win2D
    /// <c>CanvasControl</c> draws into a DirectComposition surface and owns no swapchain, so DWM
    /// composes it and <b>the app presents nothing of its own</b>. Measured, elevated, on this
    /// machine: PresentMon captured 451 presents across <c>dwm.exe</c> and <c>WindowsTerminal.exe</c>
    /// (which does own a swapchain) and <b>exactly zero</b> for ours, through sustained motion in the
    /// grid. A grid whose frames cannot be counted cannot be held to a frame budget, and §7 says "it
    /// feels alright" is not evidence.
    /// <para>
    /// The alternative was to time <c>CompositionTarget.Rendering</c> from inside the app, and that is
    /// the trap: it reports when the <i>UI thread</i> ticked, not when a frame was <i>presented</i>,
    /// exactly the confidently-wrong instrument §7 was written about (<c>gfxinfo</c>'s jank ratio and
    /// <c>mpdecimate</c> both lied on Android for the same reason).
    /// </para>
    /// <para>
    /// With a swapchain of our own, the grid shows up in PresentMon as <c>Composed: Flip</c>, and its
    /// present timestamps are the real thing. The draw code did not change: a
    /// <c>CanvasDrawingSession</c> is a <c>CanvasDrawingSession</c>.
    /// </para>
    /// </remarks>
    private readonly CanvasSwapChainPanel _panel = new();

    private CanvasSwapChain? _swapChain;
    private readonly CalendarTrace _trace = new();
    private readonly Dictionary<int, PagePaint> _pages = [];

    /// <summary>Set when something has changed and the grid owes the screen a frame.</summary>
    private bool _dirty = true;

    private CalendarSurfaceState _state = new(12, CalendarUnits.DaysInWeek);
    private CalendarSurfaceDriver _driver = null!;
    private CalendarGestureOwner _owner = null!;
    private TextLayoutCache? _text;
    private SurfaceTheme _theme = new(dark: false);
    private SurfaceStrings _strings = SurfaceStrings.Of(true, CultureInfo.CurrentCulture);

    private bool _ticking;
    private long _lastTick;

    /// <summary>
    /// The ids of the touch contacts we currently hold a capture on.
    /// </summary>
    /// <remarks>
    /// A set, not a bool, because a pinch is two contacts and each must be captured and released on
    /// its own id, see the pointer section. Ticking continues while this is non-empty, so a finger
    /// resting still on the glass keeps the frame loop alive.
    /// </remarks>
    private readonly HashSet<uint> _captured = [];

    /// <summary>A recentre asked for before there was a viewport to compute it against.</summary>
    private bool _pendingRecentre;

    /// <summary>The shape the pending recentre frames for, work week frames on Monday, the rest on
    /// the focus day (<see cref="CalendarPager.FramingColumn"/>).</summary>
    private CalendarMode _recentreMode = CalendarModes.Default;

    /// <summary>The day the pending recentre frames on, today, or a specific day tapped in the month
    /// grid. Null means today.</summary>
    private DateOnly? _recentreFocus;

    /// <summary>The spoken nodes, and what they were built for. They must be STABLE across a tree
    /// walk, see <see cref="CalendarItemPeer"/>.</summary>
    private readonly List<AutomationPeer> _spoken = [];

    private (int Week, bool Seam, bool Expanded, PagePaint Anchor, PagePaint Next) _spokenKey =
        (int.MinValue, false, false, PagePaint.Empty, PagePaint.Empty);

    /// <summary>The weeks on screen, at most two. Reused, because this is rebuilt every frame of a
    /// fling.</summary>
    private readonly List<StripPage> _strip = new(2);

    internal CalendarSurface()
    {
        _driver = new CalendarSurfaceDriver(_state);
        _owner = new CalendarGestureOwner(
            _state,
            _driver,
            () => Viewport(),
            TouchSlop,
            onZoomSettled: OnZoomSettled,
            onTap: OnTap);

        Content = _panel;

        // The grid owns every pointer that touches it. Nothing below it gets a look.
        IsTabStop = true;
        _panel.PointerPressed += OnPointerPressed;
        _panel.PointerMoved += OnPointerMoved;
        _panel.PointerReleased += OnPointerReleased;
        _panel.PointerCanceled += OnPointerCancelled;
        _panel.PointerCaptureLost += OnPointerCancelled;
        _panel.PointerWheelChanged += OnPointerWheel;

        Loaded += (_, _) => { EnsureSwapChain(); Invalidate(); };
        SizeChanged += (_, _) => { EnsureSwapChain(); Clamp(); Invalidate(); };
        Unloaded += OnUnloaded;
        ActualThemeChanged += (_, _) => Rebuild();
    }

    /// <summary>
    /// The display's DIP→physical-pixel factor, read live so it follows a move to another monitor.
    /// </summary>
    /// <remarks>
    /// From <see cref="Microsoft.UI.Xaml.XamlRoot.RasterizationScale"/>, the <b>same</b> number
    /// <see cref="EnsureSwapChain"/> feeds the swapchain's DPI (not <c>UIElement.RasterizationScale</c>,
    /// which is a per-element override, not the display factor). The accessibility overlay needs it too:
    /// its rects come from the grid's DIP geometry, but UIA reports screen rectangles in physical
    /// pixels, so <see cref="CalendarItemPeer"/> scales by this (<see cref="GridRect.ToScreen"/>).
    /// Defaults to <c>1</c> before the control has a <see cref="UIElement.XamlRoot"/>.
    /// </remarks>
    internal double DisplayScale => XamlRoot?.RasterizationScale ?? 1d;

    /// <summary>Creates or resizes the swapchain the grid presents into.</summary>
    /// <remarks>
    /// Sized in DIPs with an explicit DPI, so the grid is crisp on a scaled display, this machine
    /// runs at 200%, where a swapchain sized in DIPs and presented at 96 DPI would be visibly soft.
    /// </remarks>
    private void EnsureSwapChain()
    {
        var w = (float)ActualWidth;
        var h = (float)ActualHeight;
        if (w <= 0f || h <= 0f)
        {
            return;
        }

        var dpi = 96f * (float)(XamlRoot?.RasterizationScale ?? 1d);
        if (_swapChain is null)
        {
            var device = CanvasDevice.GetSharedDevice();
            _swapChain = new CanvasSwapChain(device, w, h, dpi);
            _panel.SwapChain = _swapChain;
            _text?.Dispose();
            _text = new TextLayoutCache(device);
            _pages.Clear();
            return;
        }

        if (Math.Abs(_swapChain.Size.Width - w) > 0.5 ||
            Math.Abs(_swapChain.Size.Height - h) > 0.5 ||
            MathF.Abs(_swapChain.Dpi - dpi) > 0.1f)
        {
            _swapChain.ResizeBuffers(w, h, dpi);
        }
    }

    // ---- What the host supplies --------------------------------------------------------------------

    /// <summary>The core's page query. Synchronous, cheap, and never touching the store or network.</summary>
    internal Func<DateOnly, uint, CalendarPage>? PageFor { get; set; }

    /// <summary>Which date a given week index anchors on.</summary>
    internal Func<int, DateOnly>? AnchorFor { get; set; }

    /// <summary>Today, from the client's clock, deliberately not in the core's snapshot.</summary>
    internal Func<DateOnly> Today { get; set; } = () => DateOnly.FromDateTime(DateTime.Now);

    /// <summary>Minutes past midnight, now. The red line is the client's, and it re-reads the clock.</summary>
    internal Func<int> NowMinutes { get; set; } =
        () => (DateTime.Now.Hour * 60) + DateTime.Now.Minute;

    /// <summary>The week's first day, from the core (<c>week_start_date</c>), never from the locale.</summary>
    internal Func<DateOnly, DateOnly>? WeekStartFor { get; set; }

    /// <summary>Raised when a pinch settles, so the host can persist the horizon and the shape.</summary>
    internal Action<CalendarMode, int>? ZoomSettled { get; set; }

    /// <summary>Raised when a tap lands on an event, with its <c>(account, event)</c>, opens the
    /// detail. A tap on empty space (or the banner) does not fire it.</summary>
    internal Action<EventOpen>? OpenEvent { get; set; }

    /// <summary>True when the app is showing 24-hour time.</summary>
    internal bool Use24Hour { get; set; } = true;

    /// <summary>The period the header names, the days actually on screen.</summary>
    internal string PeriodTitle { get; private set; } = string.Empty;

    /// <summary>Raised whenever the visible period changes, so the header can follow.</summary>
    internal Action? PeriodChanged { get; set; }

    // ---- Seeding and invalidation ------------------------------------------------------------------

    /// <summary>Re-seeds the grid from the core's persisted display settings.</summary>
    internal void Apply(DisplaySettings display)
    {
        Use24Hour = display.TimeFormat == TimeFormat.TwentyFourHour;
        _state.ResetHours(display.VisibleHours);
        var mode = display.Layout.ToMode();
        _state.ResetDays(mode.GridColumns());
        _strings = SurfaceStrings.Of(Use24Hour, CultureInfo.CurrentCulture);
        Rebuild();
    }

    /// <summary>
    /// The core's calendar data changed. Throw the painted pages away and re-pull.
    /// </summary>
    /// <remarks>
    /// This is what <c>Surface::Calendar</c> means now: not a snapshot to read, but "re-pull whatever
    /// you are showing". One snapshot slot cannot hold five pages, and two quick swipes would race
    /// (§5).
    /// </remarks>
    internal void Rebuild()
    {
        _pages.Clear();
        _text?.Clear();
        _theme.Dispose();
        _theme = new SurfaceTheme(ActualTheme == ElementTheme.Dark);
        Invalidate();
    }

    /// <summary>
    /// Scrolls the grid to show today, and the hours around now.
    /// </summary>
    /// <remarks>
    /// <b>Deferred, not applied here.</b> This is nearly always called before the control has been
    /// laid out, from the nav click that reveals it, and at that point <c>ActualWidth</c> and
    /// <c>ActualHeight</c> are still zero. Every metric derived from them is then zero too, and the
    /// scroll clamps to <c>0</c>: the grid opens at midnight, on column zero. It looks like a scroll
    /// bug and is really a lifecycle one.
    /// </remarks>
    internal void Recentre(CalendarMode mode, DateOnly? focus = null)
    {
        _recentreMode = mode;
        _recentreFocus = focus;
        // The strip is about to be teleported, so whatever was moving it is void.
        _driver.Stop();
        _state.ResetWeek();
        _pendingRecentre = true;
        Invalidate();
    }

    /// <summary>Applies a deferred recentre, once there is a viewport to compute it against.</summary>
    private void ApplyRecentre(SurfaceMetrics m)
    {
        if (!_pendingRecentre || m.Width <= 0f || m.Height <= 0f)
        {
            return;
        }
        _pendingRecentre = false;

        // ~90 minutes of context above "now", so the grid does not open with this morning already off
        // the top of the screen.
        _state.ScrollTo(
            m.HourHeight * (Math.Max(NowMinutes() - 90, 0) / CalendarUnits.MinutesInHour),
            m);

        // The day the grid frames on, today, or a specific day tapped in the month grid.
        var focus = _recentreFocus ?? Today();
        var weekStart = WeekStartFor?.Invoke(focus) ?? focus;
        _state.FrameColumn(CalendarPager.FramingColumn(_recentreMode, focus, weekStart), m);
    }

    /// <summary>
    /// Steps the grid by <paramref name="days"/>, the header's <c>&lt;</c> and <c>&gt;</c>.
    /// </summary>
    /// <remarks>
    /// The keyboard-and-mouse way to do what a swipe does, and the <i>only</i> way for the many mice
    /// that have no horizontal wheel at all. It animates rather than jumps, so the days visibly travel
    /// and the user can see which way they went.
    /// </remarks>
    internal void StepDays(int days)
    {
        _driver.SlideDays(days, _state.Metrics(Viewport()));
        Invalidate();
    }

    // The pointer stream, every touch contact, the wheel, the tap, and the fault-guard that keeps a
    // stray exception from crashing the app or stranding a capture, lives in CalendarSurface.Input.cs.
    // Split out only to stay under the 500-line file cap; it is one class.

    // ---- The clock ---------------------------------------------------------------------------------

    /// <summary>
    /// Drives the flings, the settles and the wheel's idle timer, a frame at a time.
    /// </summary>
    /// <remarks>
    /// Attached only while something is actually moving. A permanently-attached
    /// <c>CompositionTarget.Rendering</c> handler is, in WinUI's own words, "similar to running an
    /// infinite animation", it forces the UI thread awake every frame, forever, on battery.
    /// </remarks>
    private void StartTicking()
    {
        if (_ticking)
        {
            return;
        }
        _ticking = true;
        // Seeded here, not on the first callback. Seeding there cost a frame on every restart, and a
        // wheel restarts the loop on every notch, so it was a frame of latency per notch.
        _lastTick = Environment.TickCount64;
        CompositionTarget.Rendering += OnRendering;
    }

    private void StopTicking()
    {
        if (!_ticking)
        {
            return;
        }
        _ticking = false;
        CompositionTarget.Rendering -= OnRendering;
    }

    /// <summary>
    /// One frame: advance whatever is moving, and, only if anything actually changed, draw and
    /// <b>present</b>.
    /// </summary>
    /// <remarks>
    /// The present is the whole point of owning a swapchain, and it is also the thing PresentMon
    /// counts. Presenting an unchanged frame would inflate the very number §7 asks us to measure,
    /// which is why this is gated on <see cref="_dirty"/> rather than run unconditionally.
    /// </remarks>
    private void OnRendering(object? sender, object e)
    {
        var now = Environment.TickCount64;

        // Clamped: a stall (a breakpoint, a GC pause, the window being dragged) must not teleport an
        // animation to its end. Better a frame of catch-up than a week that jumps.
        var dt = Math.Clamp((now - _lastTick) / 1000f, 0f, 0.064f);
        _lastTick = now;

        var m = _state.Metrics(Viewport());
        _owner.Tick(dt);
        _driver.Tick(dt, m);

        var busy = _driver.IsAnimating || _owner.NeedsTick || _captured.Count > 0;
        if (busy)
        {
            _dirty = true;
        }

        if (_dirty)
        {
            _dirty = false;
            DrawFrame();
        }

        if (busy)
        {
            return;
        }

        // Nothing is moving. Spend the idle frames painting the pages a flick will land on next, one
        // per frame, so a page turn never builds a week's paint mid-fling (§7). This presents nothing
        // It only fills the cache, and it keeps the tick loop alive until the halo is full, then
        // stops. A new gesture makes the grid busy again and takes priority (this branch is skipped),
        // so the prebuild never competes with motion.
        if (!PrebuildAhead())
        {
            StopTicking();
        }
    }

    /// <summary>Marks the grid as owing the screen a frame, and makes sure someone will draw it.</summary>
    private void Invalidate()
    {
        _dirty = true;
        StartTicking();
    }

    private void Clamp() => _state.ClampScroll(_state.Metrics(Viewport()));

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        StopTicking();
        _text?.Dispose();
        _text = null;
        _theme.Dispose();
        _panel.SwapChain = null;
        _swapChain?.Dispose();
        _swapChain = null;
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new CalendarSurfaceAutomationPeer(this);
}
