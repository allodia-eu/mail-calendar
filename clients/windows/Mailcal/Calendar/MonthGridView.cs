// The month grid, a different layout, not the time grid with more columns (docs/calendar.md §2).
//
// A month cell has no hour axis and no overlap solving, only a list of what happens that day. So,
// exactly as Android's month is a *composed* grid rather than the drawn canvas, this is plain WinUI:
// a 6×7 grid of cells, composed once per month (and re-laid on resize). The per-frame budget §7
// guards is a property of the TIME grid's pinch and fling; the month has neither, so composition is
// the right tool and it comes with hit-testing, theming and accessibility for free.
//
// The core owns everything but the height: it returns exactly 42 cells (six weeks, so the grid never
// changes height as you page), each cell's events already ordered, and the calendar colours resolved.
// The one client decision is how many chips fit before "+N more", a question of how tall a cell is
// on THIS window, and the overflow row only earns its place when it stands for more than it displaces
// (§4): with capacity C and more than C events it draws C-1 and counts the rest, never a "+1" that
// hides one event.
using System;
using System.Globalization;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.UI;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>The drawn-once month grid: 6×7 cells, "+N more", loading state.</summary>
internal sealed class MonthGridView : UserControl
{
    private const int WeekRows = 6; // always six weeks, so the grid does not change height as you page
    private const int Cols = CalendarUnits.DaysInWeek;
    private const double HeaderRowHeight = 24;
    private const double DateNumberHeight = 22;
    private const double ChipHeight = 18;

    private readonly Grid _host = new();
    private readonly Grid _root = new();
    private readonly ProgressRing _loading = new()
    {
        IsActive = false,
        Visibility = Visibility.Collapsed,
        Width = 32,
        Height = 32,
        HorizontalAlignment = HorizontalAlignment.Center,
        VerticalAlignment = VerticalAlignment.Center,
    };

    private MonthPage? _page;
    private DateOnly _today;
    private CultureInfo _culture = CultureInfo.CurrentCulture;
    private bool _use24Hour = true;

    /// <summary>Raised when a day cell is tapped, so the host can jump to that day in the time grid.</summary>
    internal Action<DateOnly>? DayPicked { get; set; }

    /// <summary>Raised when an event chip is tapped, with its <c>(account, event)</c>, opens the
    /// detail. A chip tap is handled before it can also fire the cell's <see cref="DayPicked"/>.</summary>
    internal Action<EventOpen>? EventPicked { get; set; }

    internal MonthGridView()
    {
        _host.Children.Add(_root);
        _host.Children.Add(_loading);
        Content = _host;
        SizeChanged += (_, _) => Render();
        ActualThemeChanged += (_, _) => Render();
    }

    /// <summary>Shows <paramref name="page"/>, the month containing its anchor.</summary>
    internal void Apply(MonthPage page, DateOnly today, CultureInfo culture, bool use24Hour)
    {
        _page = page;
        _today = today;
        _culture = culture;
        _use24Hour = use24Hour;
        Render();
    }

    // How many chips fit in a cell on this window, the client's one decision (§4). A sensible default
    // before the first layout (ActualHeight is still zero then); SizeChanged re-renders with the real
    // height a frame later.
    private int ChipCapacity()
    {
        if (ActualHeight <= 0)
        {
            return 3;
        }
        var cellHeight = (ActualHeight - HeaderRowHeight) / WeekRows;
        var chipArea = cellHeight - DateNumberHeight - 4;
        return Math.Max(0, (int)(chipArea / ChipHeight));
    }

    // The contract's rule: show everything if it fits, else draw capacity-1 and count the rest, the
    // "+N more" row must stand for more than the one slot it costs (§4).
    private static int ChipsShown(int total, int capacity) =>
        total <= capacity ? total : Math.Max(capacity - 1, 0);

    private void Render()
    {
        _root.Children.Clear();
        _root.ColumnDefinitions.Clear();
        _root.RowDefinitions.Clear();

        if (_page is not { } page)
        {
            return;
        }

        // `false` does not mean "no events", it means the engine has not looked yet (§4). Say so,
        // rather than draw a confidently empty month.
        var materialized = page.IsMaterialized;
        _loading.IsActive = !materialized;
        _loading.Visibility = materialized ? Visibility.Collapsed : Visibility.Visible;

        if (page.Cells.Length < WeekRows * Cols)
        {
            return; // the core guarantees 42; guard rather than throw on a malformed page
        }

        for (var c = 0; c < Cols; c++)
        {
            _root.ColumnDefinitions.Add(new ColumnDefinition());
        }
        _root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        for (var r = 0; r < WeekRows; r++)
        {
            _root.RowDefinitions.Add(new RowDefinition());
        }

        var dark = ActualTheme == ElementTheme.Dark;

        // Weekday headings taken from the first row's own dates, so they cannot disagree with the
        // columns the core laid out (which start on the persisted week-start day).
        for (var c = 0; c < Cols; c++)
        {
            var d = ParseDate(page.Cells[c].Date);
            var head = new TextBlock
            {
                Text = _culture.DateTimeFormat.AbbreviatedDayNames[(int)d.DayOfWeek],
                HorizontalAlignment = HorizontalAlignment.Center,
                Opacity = 0.7,
                FontSize = 12,
                Margin = new Thickness(0, 0, 0, 4),
            };
            Grid.SetRow(head, 0);
            Grid.SetColumn(head, c);
            _root.Children.Add(head);
        }

        var capacity = ChipCapacity();
        for (var i = 0; i < WeekRows * Cols; i++)
        {
            var el = BuildCell(page, page.Cells[i], dark, capacity);
            Grid.SetRow(el, 1 + (i / Cols));
            Grid.SetColumn(el, i % Cols);
            _root.Children.Add(el);
        }
    }

    private FrameworkElement BuildCell(MonthPage page, MonthCell cell, bool dark, int capacity)
    {
        var date = ParseDate(cell.Date);
        var isToday = date == _today;

        var stack = new StackPanel { Spacing = 2 };
        stack.Children.Add(DateNumber(date, cell.InMonth, isToday));

        var total = cell.Chips.Length;
        var shown = ChipsShown(total, capacity);
        for (var j = 0; j < shown; j++)
        {
            stack.Children.Add(Chip(page, cell.Chips[j], dark));
        }

        var hidden = total - shown;
        if (hidden > 0)
        {
            stack.Children.Add(new TextBlock
            {
                Text = L10n.CalendarAllDayMore(hidden),
                FontSize = 11,
                Opacity = 0.7,
                Margin = new Thickness(2, 0, 0, 0),
            });
        }

        var border = new Border
        {
            BorderBrush = new SolidColorBrush(dark
                ? Color.FromArgb(255, 48, 48, 52)
                : Color.FromArgb(255, 232, 232, 236)),
            BorderThickness = new Thickness(0.5),
            Padding = new Thickness(4, 3, 4, 3),
            Child = stack,
        };

        // Tappable, jump to this day in the time grid. Tapped (not a Button) keeps the today
        // highlight and the dim-out reading cleanly, without a control template fighting them.
        border.Tapped += (_, _) => DayPicked?.Invoke(date);
        AutomationProperties.SetName(
            border,
            total > 0 ? $"{date:D}, {total}" : $"{date:D}");
        return border;
    }

    private UIElement DateNumber(DateOnly date, bool inMonth, bool isToday)
    {
        var text = new TextBlock
        {
            Text = date.Day.ToString(_culture),
            FontSize = 12,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            // Dim the leading/trailing days of the neighbouring months, so the 1st of next month does
            // not look like part of this one (§ the core's `in_month`).
            Opacity = inMonth ? 1.0 : 0.4,
        };

        if (!isToday)
        {
            text.HorizontalAlignment = HorizontalAlignment.Left;
            return text;
        }

        // Today: a filled accent disc, the same attention the time grid gives the current column.
        text.FontWeight = FontWeights.SemiBold;
        text.Foreground = new SolidColorBrush(Colors.White);
        return new Border
        {
            Width = 20,
            Height = 20,
            CornerRadius = new CornerRadius(10),
            HorizontalAlignment = HorizontalAlignment.Left,
            Background = new SolidColorBrush((Color)Application.Current.Resources["SystemAccentColor"]),
            Child = text,
        };
    }

    private UIElement Chip(MonthPage page, MonthChip chip, bool dark)
    {
        var swatch = CalendarColors.SwatchFor(page.Calendars, chip.Account, chip.Calendar, dark);
        var title = string.IsNullOrEmpty(chip.Title) ? L10n.EventNoTitle() : chip.Title;
        // An invitation nobody has answered is a hold, not a commitment: faded, dashed and hatched
        // (CalendarHold), and its spoken label says so, the drawing alone is invisible to a screen
        // reader (docs/calendar.md §4).
        var awaiting = InvitationFormat.IsAwaitingResponse(chip.Participation);
        var edge = CalendarColors.Parse(swatch.Border);

        UIElement element;
        if (chip.AllDay)
        {
            // A filled bar, the whole day is this event's colour.
            var bar = new Border
            {
                Height = ChipHeight - 2,
                CornerRadius = new CornerRadius(3),
                Padding = new Thickness(5, 0, 5, 0),
                Background = new SolidColorBrush(
                    CalendarHold.Fade(CalendarColors.Parse(swatch.Background), awaiting)),
                Child = new TextBlock
                {
                    Text = title,
                    FontSize = 11,
                    MaxLines = 1,
                    TextTrimming = TextTrimming.CharacterEllipsis,
                    VerticalAlignment = VerticalAlignment.Center,
                    Foreground = new SolidColorBrush(CalendarColors.Parse(swatch.Text)),
                },
            };
            element = CalendarHold.Compose(bar, edge, corner: 3, awaiting, ChipHeight - 2);
        }
        else
        {
            // A timed event, a coloured dot, its start time, and its title.
            var row = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 5,
                Height = ChipHeight - 2,
            };
            row.Children.Add(new Border
            {
                Width = 8,
                Height = 8,
                CornerRadius = new CornerRadius(4),
                VerticalAlignment = VerticalAlignment.Center,
                Background = new SolidColorBrush(
                    CalendarHold.Fade(CalendarColors.Parse(swatch.Background), awaiting)),
            });
            var time = CalendarFormat.ClockTime((int)chip.StartMinutes, _use24Hour, _culture);
            row.Children.Add(new TextBlock
            {
                Text = $"{time} {title}",
                FontSize = 11,
                MaxLines = 1,
                TextTrimming = TextTrimming.CharacterEllipsis,
                VerticalAlignment = VerticalAlignment.Center,
            });
            element = CalendarHold.Compose(row, edge, corner: 3, awaiting, ChipHeight - 2);
        }

        // A tap opens the event's detail, and is marked handled so it does not also fire the cell's
        // DayPicked and jump into the day zoom instead.
        element.Tapped += (_, e) =>
        {
            e.Handled = true;
            EventPicked?.Invoke(new EventOpen(chip.Account, chip.Event, chip.OccurrenceStart));
        };
        AutomationProperties.SetName(
            (DependencyObject)element,
            InvitationFormat.SpokenWithHold(
                title, L10n.A11yInvitationAwaitingResponse(), chip.Participation));
        return element;
    }

    private static DateOnly ParseDate(string iso) =>
        DateOnly.TryParseExact(iso, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var d)
            ? d
            : DateOnly.FromDateTime(DateTime.Now);
}
