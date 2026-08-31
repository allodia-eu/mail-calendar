// The meeting-day preview: one day of the user's own calendar, drawn under the invitation card.
//
// Laid out by the same `calendar::grid::build` every calendar surface uses, the core hands this one
// already solved, in InvitationCard.Preview, so the preview and the real grid cannot disagree about
// where a block sits. The client only multiplies: a wall-clock minute by an hour height, a column
// fraction by a width (docs/calendar.md §1).
//
// COMPOSED, not painted on a canvas, the opposite of CalendarSurface, and for the reason
// MonthGridView gives: §7's per-frame budget is a property of the time grid's pinch and fling, and
// this picture has neither. It is laid out once per card and then sits there, so composition is the
// right tool and it brings theming and DPI with it.
//
// The hour height is derived from the span rather than fixed, so every block on that day fits: the
// preview never clips, which is what lets it stay a picture with no "and 2 more" caveat
// (docs/calendar.md §4, nothing is hidden without saying so).
using System.Globalization;
using Allodia.Mailcal.Calendar;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>One already-loaded day of the calendar, as the invitation card's clash picture.</summary>
internal sealed class InvitationPreviewGrid : UserControl
{
    /// <summary>The hour ruler's column.</summary>
    private const double Gutter = 34;

    /// <summary>One stacked all-day bar.</summary>
    private const double LaneHeight = 18;

    /// <summary>The grid's height for the span currently rendered, see
    /// <see cref="InvitationFormat.PreviewHeight"/>, which is why it is derived rather than a
    /// constant. Held as a field because the row definition, the clip and the chips all need it and
    /// only Render knows the span.</summary>
    private double _gridHeight = InvitationFormat.PreviewHeight(0);

    /// <summary>The chip's corner, matching the month grid's, one calendar, one shape.</summary>
    private const double ChipCorner = 3;

    private readonly Grid _root = new();
    private readonly Grid _bands = new() { Margin = new Thickness(Gutter, 0, 0, 0) };
    private readonly Canvas _ruler = new() { Width = Gutter };
    private readonly Canvas _day = new();

    private InvitationPreview? _preview;
    private MinuteSpan _meeting;
    private bool _use24Hour = true;
    private CultureInfo _culture = CultureInfo.CurrentCulture;

    internal InvitationPreviewGrid()
    {
        _root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        _root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(_gridHeight) });
        Grid.SetRow(_bands, 0);
        _root.Children.Add(_bands);

        var body = new Grid();
        body.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(Gutter) });
        body.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        Grid.SetColumn(_ruler, 0);
        Grid.SetColumn(_day, 1);
        // The blocks are positioned from midnight and the whole day slid up, exactly as the real grid
        // does it, same multiplication, no second solver, so the overhang has to be clipped away.
        _day.Clip = new RectangleGeometry();
        _day.SizeChanged += (_, _) => Render();
        body.Children.Add(_ruler);
        body.Children.Add(_day);
        Grid.SetRow(body, 1);
        _root.Children.Add(body);

        Content = _root;
        ActualThemeChanged += (_, _) => Render();
    }

    /// <summary>Shows <paramref name="preview"/>, the meeting's own day, as the core laid it out.</summary>
    /// <param name="preview">The solved day: blocks, all-day bars, and the zone it was solved in.</param>
    /// <param name="meeting">
    /// The meeting's own wall-clock window, included in the hour span explicitly rather than relying
    /// on the hold the provider scheduled: a bare IMAP+SMTP account has no auto-scheduling server, so
    /// nothing lands on the grid and the meeting would otherwise be off the top of its own preview.
    /// </param>
    /// <param name="use24Hour">The user's clock setting, not the culture's default.</param>
    /// <param name="culture">The formatting culture the app's language choice pinned.</param>
    internal void Apply(
        InvitationPreview preview, MinuteSpan meeting, bool use24Hour, CultureInfo culture)
    {
        _preview = preview;
        _meeting = meeting;
        _use24Hour = use24Hour;
        _culture = culture;
        Render();
    }

    private void Render()
    {
        _bands.Children.Clear();
        _bands.RowDefinitions.Clear();
        _ruler.Children.Clear();
        _day.Children.Clear();
        if (_preview is not { } preview)
        {
            return;
        }
        var dark = ActualTheme == ElementTheme.Dark;

        var others = new List<MinuteSpan>(preview.Timed.Length);
        foreach (var segment in preview.Timed)
        {
            others.Add(new MinuteSpan((int)segment.StartMinutes, (int)segment.EndMinutes));
        }
        var span = InvitationFormat.PreviewSpan(_meeting, others);
        // Tall enough that the meeting's own block can carry its title, see
        // InvitationFormat.PreviewHeight, which is why this is derived from the span. The row was
        // sized from the same call at construction, so it only has to be re-set when it changes.
        _gridHeight = InvitationFormat.PreviewHeight(span.Count);
        _root.RowDefinitions[1].Height = new GridLength(_gridHeight);
        var hourHeight = _gridHeight / span.Count;

        RenderBands(preview, dark);
        RenderRuler(span, hourHeight);

        var dayWidth = _day.ActualWidth;
        if (_day.Clip is RectangleGeometry clip)
        {
            clip.Rect = new Rect(0, 0, dayWidth, _gridHeight);
        }
        if (dayWidth <= 0)
        {
            return; // Before the first layout pass; SizeChanged re-renders at the real width.
        }
        RenderGridLines(span, hourHeight, dayWidth);
        RenderBlocks(preview, span, hourHeight, dayWidth, dark);
    }

    // One day, so a bar spans the full width and the banner is as tall as the core's lane count. No
    // "+N" overflow: a single day's all-day events fit, and capping them here would hide one.
    private void RenderBands(InvitationPreview preview, bool dark)
    {
        for (var lane = 0; lane < preview.AllDayLanes; lane++)
        {
            _bands.RowDefinitions.Add(new RowDefinition { Height = new GridLength(LaneHeight) });
        }
        foreach (var band in preview.AllDay)
        {
            if (band.Lane >= preview.AllDayLanes)
            {
                continue; // Defensive: a lane the banner was not sized for would draw outside it.
            }
            var chip = Chip(
                band.Title,
                CalendarColors.SwatchFor(NoCalendars, band.Account, band.Calendar, dark),
                InvitationFormat.IsAwaitingResponse(band.Participation),
                LaneHeight - 2);
            chip.HorizontalAlignment = HorizontalAlignment.Stretch;
            Grid.SetRow(chip, (int)band.Lane);
            _bands.Children.Add(chip);
        }
    }

    /// <summary>The hour labels down the left edge, every <c>PreviewStride</c> hours.</summary>
    private void RenderRuler(HourSpan span, double hourHeight)
    {
        var stride = InvitationFormat.PreviewStride(hourHeight);
        var ink = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"];
        for (var hour = span.First; hour < span.Last; hour++)
        {
            if ((hour - span.First) % stride != 0)
            {
                continue;
            }
            var text = CalendarFormat.HourLabel(hour, _use24Hour, _culture);
            if (string.IsNullOrEmpty(text))
            {
                continue; // Midnight's label is deliberately empty (CalendarFormat.HourLabel).
            }
            var label = new TextBlock
            {
                Text = text,
                FontSize = 9,
                Width = Gutter - 4,
                TextAlignment = TextAlignment.Right,
                Foreground = ink,
            };
            // A label straddles its own gridline, as the full grid's ruler does, except the first,
            // which has no line above it and would hang off the top.
            Canvas.SetTop(label, Math.Max((hourHeight * (hour - span.First)) - 6, 0));
            _ruler.Children.Add(label);
        }
    }

    private void RenderGridLines(HourSpan span, double hourHeight, double dayWidth)
    {
        var stroke = (Brush)Application.Current.Resources["DividerStrokeColorDefaultBrush"];
        for (var hour = span.First; hour <= span.Last; hour++)
        {
            var line = new Rectangle { Width = dayWidth, Height = 1, Fill = stroke };
            Canvas.SetTop(line, hourHeight * (hour - span.First));
            _day.Children.Add(line);
        }
    }

    private void RenderBlocks(
        InvitationPreview preview, HourSpan span, double hourHeight, double dayWidth, bool dark)
    {
        // The blocks position themselves from midnight, as they do on the real grid; the whole day is
        // laid out and slid up so the span starts at the top. Same multiplication, no second solver.
        var offset = hourHeight * span.First;
        foreach (var segment in preview.Timed)
        {
            var columns = Math.Max((int)segment.Columns, 1);
            var columnWidth = dayWidth / columns;
            var top = (hourHeight * (segment.StartMinutes / CalendarUnits.MinutesInHour)) - offset;
            var bottom = (hourHeight * (segment.EndMinutes / CalendarUnits.MinutesInHour)) - offset;
            var chip = Chip(
                segment.Title,
                CalendarColors.SwatchFor(NoCalendars, segment.Account, segment.Calendar, dark),
                InvitationFormat.IsAwaitingResponse(segment.Participation),
                Math.Max(bottom - top - 1, 1));
            chip.Width = Math.Max(columnWidth - 1, 1);
            Canvas.SetTop(chip, top);
            Canvas.SetLeft(chip, columnWidth * segment.Column);
            _day.Children.Add(chip);
        }
    }

    /// <summary>
    /// One preview block or all-day bar: a rounded fill, then a clipped title.
    /// </summary>
    /// <remarks>
    /// A <b>hold</b>, an invitation nobody has answered, is drawn by <see cref="CalendarHold"/>, the
    /// same call the week grid, the all-day bar and the month chip make, so the four surfaces cannot
    /// end up describing the same record differently. The drawing is not the disclosure on its own (a
    /// dashed edge is invisible to a screen reader); on this surface the card's own answer line and
    /// its conflict sentence carry that, which is why the preview needs no spoken label of its own.
    /// </remarks>
    private static FrameworkElement Chip(string title, Swatch swatch, bool awaiting, double height)
    {
        var edge = CalendarColors.Parse(swatch.Border);
        var chip = new Grid { Height = height };
        chip.Children.Add(new Rectangle
        {
            RadiusX = ChipCorner,
            RadiusY = ChipCorner,
            Fill = new SolidColorBrush(CalendarHold.Fade(CalendarColors.Parse(swatch.Background), awaiting)),
        });
        if (height >= InvitationFormat.MinimumTitledHeight)
        {
            chip.Children.Add(new TextBlock
            {
                Text = string.IsNullOrEmpty(title) ? L10n.EventNoTitle() : title,
                FontSize = 9,
                MaxLines = 1,
                TextTrimming = TextTrimming.CharacterEllipsis,
                VerticalAlignment = VerticalAlignment.Top,
                Margin = new Thickness(4, 1, 2, 0),
                Foreground = new SolidColorBrush(
                    awaiting ? edge : CalendarColors.Parse(swatch.Text)),
            });
        }
        return CalendarHold.Compose(chip, edge, ChipCorner, awaiting, height);
    }

    // The preview carries no calendar list, it is one already-loaded day, not a page to page
    // through, so every block falls back to the neutral swatch. The preview is about *when*, not
    // about which calendar.
    private static readonly IReadOnlyList<CalendarRow> NoCalendars = Array.Empty<CalendarRow>();
}
