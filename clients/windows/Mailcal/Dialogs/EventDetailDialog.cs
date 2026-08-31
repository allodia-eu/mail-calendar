// The event detail, what a tap on any event opens, the Windows twin of Android's EventDetailScreen
// and Apple's EventDetailView. Title, the time range in the event's own wall clock, the calendar with
// its colour dot, location, notes, and the reminder/recurrence summaries. Edit and Delete are the
// dialog's Primary and Secondary buttons, and are offered only for a writable calendar's event
// (canWrite): an affordance that can never fire is just a mystery. A read-only event
// opens with a Close button alone.
//
// The dialog returns which action the user chose via its ContentDialogResult (Primary = edit,
// Secondary = delete, None = close); the caller (CalendarView) opens the editor or confirms the
// delete. Built imperatively in code-behind, like the calendar manager and the editor.
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>A read-only event detail with Edit / Delete actions (gated on <c>canWrite</c>).</summary>
public sealed class EventDetailDialog : ContentDialog
{
    private readonly EventDetail _detail;
    private readonly CalendarRow[] _calendars;

    /// <summary>Builds the detail over the core's <paramref name="detail"/> read.</summary>
    internal EventDetailDialog(EventDetail detail, MailboxModel model)
    {
        _detail = detail;
        _calendars = model.Calendars();

        Title = detail.Title.Length == 0 ? L10n.EventNoTitle() : detail.Title;
        CloseButtonText = L10n.ActionClose();
        DefaultButton = ContentDialogButton.Close;

        // Edit + Delete only for a writable calendar's event.
        if (detail.CanWrite)
        {
            PrimaryButtonText = L10n.ActionEdit();
            SecondaryButtonText = L10n.ActionDelete();
            DefaultButton = ContentDialogButton.Primary;
        }

        Content = new ScrollViewer { Content = BuildBody(), MinWidth = 340, MaxHeight = 460 };
    }

    private StackPanel BuildBody()
    {
        var culture = CultureInfo.CurrentCulture;
        var panel = new StackPanel { Spacing = 6, Width = 320 };

        panel.Children.Add(new TextBlock
        {
            Text = DetailTime(_detail, culture),
            Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
        });
        if (_detail.Timezone.Length > 0)
        {
            panel.Children.Add(Secondary(_detail.Timezone));
        }

        // Calendar, with its colour dot.
        var row = _calendars.FirstOrDefault(c => c.Account == _detail.Account && c.Id == _detail.Calendar);
        var calendarLine = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 10,
            Margin = new Thickness(0, 8, 0, 0),
        };
        if (row is not null)
        {
            calendarLine.Children.Add(new Border
            {
                Width = 14,
                Height = 14,
                CornerRadius = new CornerRadius(7),
                VerticalAlignment = VerticalAlignment.Center,
                Background = new SolidColorBrush(CalendarColors.Parse(
                    (ActualTheme == ElementTheme.Dark ? row.Color.Dark : row.Color.Light).Background)),
            });
        }
        calendarLine.Children.Add(new TextBlock
        {
            Text = row?.Name ?? _detail.Calendar,
            VerticalAlignment = VerticalAlignment.Center,
        });
        panel.Children.Add(calendarLine);

        if (!string.IsNullOrWhiteSpace(_detail.Location))
        {
            panel.Children.Add(DetailRow(L10n.EventLocation(), _detail.Location!));
        }
        if (!string.IsNullOrWhiteSpace(_detail.Notes))
        {
            panel.Children.Add(DetailRow(L10n.EventNotes(), _detail.Notes!));
        }
        panel.Children.Add(DetailRow(L10n.EventReminder(), CalendarEventText.Reminder(_detail.ReminderMinutes)));
        panel.Children.Add(DetailRow(
            L10n.EventRepeat(),
            EventRepeatText.Summary(_detail.RepeatSummary, _detail.IsRecurring, culture),
            "EventRepeatValue"));

        // No heading at all for an appointment nobody was invited to, an empty "Attendees" label
        // would read as "we looked and found none", a different statement from "this is not a
        // meeting".
        if (_detail.Attendees.Length > 0)
        {
            panel.Children.Add(AttendeeBlock(_detail.Attendees));
        }

        return panel;
    }

    /// <summary>The attendee list under its heading, shared shape with the editor's copy.</summary>
    /// <remarks>
    /// The heading carries an AutomationId so the UI suite can assert on it rather than on the
    /// localised word "Attendees", which would make the test pass or fail by whichever language the
    /// developer's app happens to be in. It sits on the <see cref="TextBlock"/> and not on the
    /// surrounding panel deliberately: a bare panel is not a control element, so UIA never shows it
    /// and an id there is silently unreachable (verified, the suite found nothing). The heading is
    /// also the thing the product rule is about, since an event nobody was invited to must show
    /// <b>no heading at all</b>, and an absence is only assertable when the thing has a handle.
    /// </remarks>
    internal static StackPanel AttendeeBlock(IReadOnlyList<EventAttendee> attendees)
    {
        var block = new StackPanel { Spacing = 2, Margin = new Thickness(0, 10, 0, 0) };
        var heading = new TextBlock
        {
            Text = L10n.EventAttendees(),
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        };
        AutomationProperties.SetAutomationId(heading, "EventAttendeesHeading");
        block.Children.Add(heading);
        foreach (var attendee in attendees)
        {
            block.Children.Add(AttendeeRow(attendee));
        }
        return block;
    }

    // One attendee: name (or address), the address + "Organiser" beneath it, and how they answered.
    // Every string is attacker-controlled, the core has already stripped control characters and
    // bidi overrides, and a TextBlock renders text, so there is nothing further to escape.
    private static Grid AttendeeRow(EventAttendee attendee)
    {
        var row = new Grid { Margin = new Thickness(0, 6, 0, 0), ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var who = new StackPanel { Spacing = 1 };
        who.Children.Add(new TextBlock
        {
            Text = AttendeeSummary.Title(attendee),
            TextWrapping = TextWrapping.Wrap,
        });
        var subtitle = AttendeeSummary.Subtitle(attendee, L10n.EventAttendeeOrganizer());
        if (subtitle.Length > 0)
        {
            who.Children.Add(Secondary(subtitle));
        }
        row.Children.Add(who);

        var answer = Secondary(CalendarEventText.AttendeeResponse(attendee.Response));
        answer.VerticalAlignment = VerticalAlignment.Top;
        Grid.SetColumn(answer, 1);
        row.Children.Add(answer);
        return row;
    }

    // `automationId` names the VALUE, not the row: a label is localised, so a test that finds this
    // row by its caption passes or fails by the language the developer's machine is in.
    private static StackPanel DetailRow(string label, string value, string? automationId = null)
    {
        var text = new TextBlock { Text = value, TextWrapping = TextWrapping.Wrap };
        if (automationId is not null)
        {
            AutomationProperties.SetAutomationId(text, automationId);
        }
        return new StackPanel
        {
            Spacing = 1,
            Margin = new Thickness(0, 10, 0, 0),
            Children =
            {
                new TextBlock { Text = label, Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"] },
                text,
            },
        };
    }

    private static TextBlock Secondary(string text) => new()
    {
        Text = text,
        Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
    };

    /// <summary>
    /// The event's time as one line, in its own wall clock. All-day shows the inclusive day(s); a timed
    /// event shows the date and a start–end range, collapsing the date when start and end share one.
    /// </summary>
    internal static string DetailTime(EventDetail detail, CultureInfo culture)
    {
        var start = EventEditorState.ParseWall(detail.Start);
        if (detail.AllDay)
        {
            // The stored end is exclusive; show the inclusive last day.
            var lastDay = EventEditorState.ParseWall(detail.End).AddDays(-1).Date;
            return lastDay == start.Date
                ? start.ToString("D", culture)
                : $"{start.ToString("D", culture)} – {lastDay.ToString("D", culture)}";
        }
        var end = EventEditorState.ParseWall(detail.End);
        if (start.Date == end.Date)
        {
            return $"{start.ToString("D", culture)}, {start.ToString("t", culture)} – {end.ToString("t", culture)}";
        }
        return $"{start.ToString("D", culture)} {start.ToString("t", culture)} – " +
            $"{end.ToString("D", culture)} {end.ToString("t", culture)}";
    }
}
