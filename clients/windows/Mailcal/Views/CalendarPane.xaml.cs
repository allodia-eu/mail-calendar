// The calendar pane (docs/calendar-pane.md): pulled from the core on the calendar's signal, on a
// settings signal and when the grid comes to rest elsewhere, and drawn whole from that one pull. The
// mini month's day seats the grid through the calendar's own layout path; a calendar row toggles,
// recolours and redraws from the next pull, keeping no copy of its own.

using System.Globalization;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.System;
using uniffi.mailcal_bindings;
using PaneRecord = uniffi.mailcal_bindings.CalendarPane;

namespace Allodia.Mailcal.Views;

/// <summary>The mini month and the calendars, beside the calendar.</summary>
public sealed partial class CalendarPane : UserControl
{
    private readonly CalendarPaneModel _state = new();
    private readonly List<Button> _days = [];
    private MailboxModel? _model;
    private CalendarView? _calendar;
    private PaneRecord? _pane;

    /// <summary>Initialises the control.</summary>
    public CalendarPane()
    {
        this.InitializeComponent();
        // The colour checks are drawn in the theme's own swatch.
        ActualThemeChanged += (_, _) => Pull();
    }

    /// <summary>Binds the pane to the model and to the calendar it sits beside.</summary>
    internal void Init(MailboxModel model, CalendarView calendar)
    {
        _model = model;
        _calendar = calendar;
        model.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(MailboxModel.CalendarVersion) or nameof(MailboxModel.DisplaySettingsVersion))
            {
                Pull();
            }
        };
        calendar.ShownDaysChanged += () =>
        {
            if (_state.Follow(calendar.Place))
            {
                Pull();
            }
        };
    }

    /// <summary>The calendar came on screen.</summary>
    internal void OnShown()
    {
        if (_calendar is not null)
        {
            _state.Follow(_calendar.Place);
        }
        Pull();
    }

    private void Pull()
    {
        if (_model is not { Destination: AppDestination.Calendar } || _calendar is null)
        {
            return;
        }
        if (_model.CalendarPane(_state.Query(_model.Today(), _calendar.Place)) is not { } pane)
        {
            return;
        }
        _pane = pane;
        var focused = FocusedId();
        DrawMonth(pane.Month);
        DrawGroups(pane.Groups);
        Refocus(focused);
    }

    private void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (_pane is not null && Math.Abs(e.NewSize.Width - e.PreviousSize.Width) > 0.5)
        {
            DrawMonth(_pane.Month);
        }
    }

    private void OnPreviousMonth(object sender, RoutedEventArgs e) => PageTo(_pane?.Month.Previous);

    private void OnNextMonth(object sender, RoutedEventArgs e) => PageTo(_pane?.Month.Next);

    private void PageTo(string? month)
    {
        if (month is null)
        {
            return;
        }
        _state.Page(month);
        Pull();
    }

    // ---- The mini month ----------------------------------------------------------------------------

    private void DrawMonth(MiniMonth month)
    {
        var culture = CultureInfo.CurrentCulture;
        MonthTitle.Text = CalendarPaneModel.MonthTitle(month, culture);

        var cell = CalendarPaneModel.CellSize(ActualWidth - 16);
        MonthGrid.Children.Clear();
        MonthGrid.ColumnDefinitions.Clear();
        MonthGrid.RowDefinitions.Clear();
        _days.Clear();
        if (cell <= 0)
        {
            return;
        }
        for (var c = 0; c < 8; c++)
        {
            MonthGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(cell) });
        }
        for (var r = 0; r <= CalendarPaneModel.Weeks; r++)
        {
            MonthGrid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(cell) });
        }

        // The initials are decoration for a screen reader: every day button speaks its whole date.
        for (var c = 0; c < month.Weekdays.Length; c++)
        {
            Place(Text(CalendarPaneModel.WeekdayInitial(month.Weekdays[c], culture), "PaneCaptionStyle", raw: true), 0, c + 1);
        }

        var today = L10n.CalendarToday();
        var shown = L10n.A11yCalendarPaneDayShown();
        for (var r = 0; r < month.Weeks.Length; r++)
        {
            var week = month.Weeks[r];
            var number = week.Number.ToString(culture);
            var label = Text(number, "PaneWeekNumberStyle", raw: false);
            AutomationProperties.SetName(label, L10n.CalendarWeekNumber(number));
            Place(label, r + 1, 0);
            for (var c = 0; c < week.Days.Length; c++)
            {
                var day = week.Days[c];
                if (CalendarPaneModel.Band(day.Shown) is { } ends)
                {
                    Place(Band(ends, cell), r + 1, c + 1);
                }
                var button = DayButton(day, cell, CalendarPaneModel.DayName(day, culture, today, shown));
                _days.Add(button);
                Place(button, r + 1, c + 1);
            }
        }
    }

    private Border Band(BandEnds ends, double cell)
    {
        var height = cell - 4;
        var start = ends.RoundStart ? height / 2 : 0;
        var end = ends.RoundEnd ? height / 2 : 0;
        return new Border
        {
            Style = (Style)Resources["PaneBandStyle"],
            Height = height,
            Margin = new Thickness(ends.RoundStart ? 2 : 0, 0, ends.RoundEnd ? 2 : 0, 0),
            CornerRadius = new CornerRadius(start, end, end, start),
        };
    }

    private Button DayButton(MiniDay day, double cell, string name)
    {
        var content = new Grid();
        if (day.IsToday)
        {
            content.Children.Add(new Ellipse { Style = (Style)Resources["PaneTodayStyle"], Width = cell - 6, Height = cell - 6 });
        }
        var style = day.IsToday ? "PaneTodayTextStyle" : day.InMonth ? "PaneDayTextStyle" : "PaneOutsideTextStyle";
        content.Children.Add(Text(day.Day.ToString(CultureInfo.CurrentCulture), style, raw: true));

        var button = new Button
        {
            Style = (Style)Resources["PaneFlatButtonStyle"],
            Width = cell,
            Height = cell,
            CornerRadius = new CornerRadius(cell / 2),
            Content = content,
            Tag = day.Date,
        };
        AutomationProperties.SetName(button, name);
        AutomationProperties.SetAutomationId(button, $"CalendarPaneDay_{day.Date}");
        button.Click += (_, _) => Seat(day.Date);
        button.KeyDown += OnDayKey;
        return button;
    }

    // A day pressed: where the core puts the grid for it, applied through the calendar's layout path.
    private void Seat(string date)
    {
        if (_model is null || _calendar is null)
        {
            return;
        }
        var at = CalendarPaneModel.ParseDate(date);
        if (_model.CalendarSeat(at, _calendar.Place.Layout) is { } seat)
        {
            _calendar.ApplySeat(seat);
        }
    }

    private void OnDayKey(object sender, KeyRoutedEventArgs e)
    {
        var (columns, rows) = e.Key switch
        {
            VirtualKey.Left => (-1, 0),
            VirtualKey.Right => (1, 0),
            VirtualKey.Up => (0, -1),
            VirtualKey.Down => (0, 1),
            _ => (0, 0),
        };
        var from = sender is Button button ? _days.IndexOf(button) : -1;
        if ((columns, rows) == (0, 0) || from < 0)
        {
            return;
        }
        var to = CalendarPaneModel.Move(from, columns, rows);
        _days[to].Focus(FocusState.Keyboard);
        e.Handled = true;
    }

    // ---- Helpers -----------------------------------------------------------------------------------

    private TextBlock Text(string text, string style, bool raw)
    {
        var block = new TextBlock { Text = text, Style = (Style)Resources[style] };
        if (raw)
        {
            AutomationProperties.SetAccessibilityView(block, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        }
        return block;
    }

    private void Place(FrameworkElement element, int row, int column)
    {
        Grid.SetRow(element, row);
        Grid.SetColumn(element, column);
        MonthGrid.Children.Add(element);
    }

    // A redraw replaces every control, so the keyboard focus is carried over by automation id.
    private string? FocusedId() =>
        XamlRoot is not null && FocusManager.GetFocusedElement(XamlRoot) is FrameworkElement focused
            && IsAncestorOf(focused)
            ? AutomationProperties.GetAutomationId(focused)
            : null;

    private void Refocus(string? id)
    {
        if (string.IsNullOrEmpty(id))
        {
            return;
        }
        var target = _days.FirstOrDefault(d => AutomationProperties.GetAutomationId(d) == id)
            ?? Groups.Children.OfType<Control>().FirstOrDefault(c => AutomationProperties.GetAutomationId(c) == id)
            ?? (Control?)(AutomationProperties.GetAutomationId(PreviousMonth) == id ? PreviousMonth : null)
            ?? (AutomationProperties.GetAutomationId(NextMonth) == id ? NextMonth : null);
        target?.Focus(FocusState.Keyboard);
    }

    private bool IsAncestorOf(DependencyObject element)
    {
        for (var at = element; at is not null; at = VisualTreeHelper.GetParent(at))
        {
            if (ReferenceEquals(at, this))
            {
                return true;
            }
        }
        return false;
    }
}
