// The event detail, what a tap on any event opens, the Windows twin of Android's EventDetailScreen
// and Apple's EventDetailView. Title, the time range in the event's own wall clock, the calendar with
// its colour dot, an invitee's answer (EventDetailDialog.Answer.cs), location, notes, the
// reminder/recurrence summaries and the attendee list. Edit and Delete are the dialog's Primary and
// Secondary buttons, and are offered only for a writable calendar's event (canWrite): an affordance
// that can never fire is just a mystery. A read-only event opens with a Close button alone.
//
// The dialog returns which action the user chose via its ContentDialogResult (Primary = edit,
// Secondary = delete, None = close, or Declining); the caller (CalendarView) opens the editor or
// confirms the delete or the decline. Built imperatively in code-behind, like the calendar manager
// and the editor.
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>A read-only event detail with Edit / Delete actions (gated on <c>canWrite</c>).</summary>
public sealed partial class EventDetailDialog : ContentDialog
{
    private readonly MailboxModel _model;
    private readonly CalendarRow[] _calendars;
    private readonly ScrollViewer _scroller = new() { MinWidth = 340, MaxHeight = 460 };
    private EventDetail _detail;

    // Built before DialogHelper mirrors the window's theme onto this dialog, so the colours are
    // handed out as brushes that follow it rather than read once (ThemePalette.cs).
    private readonly ThemeBrushes _brushes;

    // The links in the notes, which the dialog's initial focus must not land on.
    private Hyperlink[] _noteLinks = [];

    // Set while Rebuild hands focus on to an answer, the one programmatic move into the content
    // this dialog makes itself.
    private bool _handingOnFocus;

    /// <summary>Builds the detail over the core's <paramref name="detail"/> read.</summary>
    internal EventDetailDialog(EventDetail detail, MailboxModel model)
    {
        _detail = detail;
        _model = model;
        _calendars = model.Calendars();
        _brushes = new ThemeBrushes(this);

        CloseButtonText = L10n.ActionClose();
        DefaultButton = ContentDialogButton.Close;

        // Edit + Delete only for a writable calendar's event.
        if (detail.CanWrite)
        {
            PrimaryButtonText = L10n.ActionEdit();
            SecondaryButtonText = L10n.ActionDelete();
            DefaultButton = ContentDialogButton.Primary;
        }

        BuildReplyBar();
        Content = new StackPanel { Children = { _replyBar, _scroller } };
        Rebuild();
        FocusManager.GettingFocus += KeepInitialFocusOffTheContent;
        model.PropertyChanged += OnModelChanged;
        Closed += (_, _) =>
        {
            FocusManager.GettingFocus -= KeepInitialFocusOffTheContent;
            model.PropertyChanged -= OnModelChanged;
        };
    }

    /// <summary>The event as last read, which a re-read after an answer moves on.</summary>
    internal EventDetail Detail => _detail;

    // Draws the detail from the current read, at the scroll it was read at. Focus inside the old
    // body, or an answer pressed from the keyboard, goes to the answer row's first live button, so
    // a keyboard user who answered is not left on a control that is gone.
    private void Rebuild()
    {
        var hadFocus = XamlRoot is { } root
            && FocusManager.GetFocusedElement(root) is DependencyObject focused
            && IsWithin(focused, _scroller);
        var offset = _scroller.VerticalOffset;
        Title = _detail.Title.Length == 0 ? L10n.EventNoTitle() : _detail.Title;
        _scroller.Content = BuildBody();
        _scroller.ChangeView(null, offset, null, disableAnimation: true);
        if (hadFocus)
        {
            _answeredFromKeyboard = true;
        }
        HandFocusToAnswers();
    }

    // Puts the focus on the first answer that can be pressed, once, after a keyboard answer.
    private void HandFocusToAnswers()
    {
        if (!_answeredFromKeyboard || FirstLiveAnswer() is not { } button)
        {
            return;
        }
        _answeredFromKeyboard = false;
        _handingOnFocus = true;
        button.Focus(FocusState.Programmatic);
        _handingOnFocus = false;
    }

    // A Hyperlink is a text element, outside the visual tree VisualTreeHelper walks.
    private static bool IsWithin(DependencyObject element, DependencyObject ancestor)
    {
        if (element is not UIElement)
        {
            return false;
        }
        for (var node = element; node is not null; node = VisualTreeHelper.GetParent(node))
        {
            if (ReferenceEquals(node, ancestor))
            {
                return true;
            }
        }
        return false;
    }

    private Button? FirstLiveAnswer() => EventAnswer.Choices
        .Select(response => _answerButtons.GetValueOrDefault(response))
        .FirstOrDefault(button => button is { IsEnabled: true });

    // A ContentDialog gives its initial focus to the first focusable element of its content: a
    // meeting's Accept, the reply question's Send, or a link in the notes. Enter would then answer a
    // meeting, email an organiser or open a page, where it should press the default button. That
    // focus is programmatic, and the only programmatic move into the content this dialog makes is
    // Rebuild's, so any other goes to the default button instead; Tab and a click still reach every
    // control. Redirected as it happens rather than corrected on Opened, which the dialog's own focus
    // can follow, and through FocusManager because a Hyperlink is not a UIElement and raises no
    // GettingFocus of its own. The parts are named in ContentDialog's own template.
    private void KeepInitialFocusOffTheContent(object? sender, GettingFocusEventArgs args)
    {
        // A move away from the content (a pressed answer going inert while it saves, or one a
        // redraw took out of the tree) is not the dialog's initial focus, and stays where WinUI
        // puts it.
        if (args.FocusState != FocusState.Programmatic
            || _handingOnFocus
            || (args.OldFocusedElement is UIElement old
                && ((old is Button retired && _retiredAnswers.Contains(retired))
                    || (Content is DependencyObject owner && IsWithin(old, owner)))))
        {
            return;
        }
        var intoContent = args.NewFocusedElement is Hyperlink link
            ? _noteLinks.Contains(link)
            : args.NewFocusedElement is DependencyObject element
                && Content is DependencyObject content
                && IsWithin(element, content);
        if (!intoContent)
        {
            return;
        }
        var name = DefaultButton == ContentDialogButton.Primary ? "PrimaryButton" : "CloseButton";
        if (GetTemplateChild(name) is Control button)
        {
            args.TrySetNewFocusedElement(button);
        }
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

        _retiredAnswers.UnionWith(_answerButtons.Values);
        _answerButtons.Clear();
        if (AnswerBlock() is { } answer)
        {
            panel.Children.Add(answer);
        }

        if (!string.IsNullOrWhiteSpace(_detail.Location))
        {
            panel.Children.Add(DetailRow(L10n.EventLocation(), _detail.Location!));
        }
        if (!string.IsNullOrWhiteSpace(_detail.Notes))
        {
            var notes = DetailRow(L10n.EventNotes(), _detail.Notes!, linked: true);
            _noteLinks = notes.Children.OfType<TextBlock>()
                .SelectMany(text => text.Inlines.OfType<Hyperlink>()).ToArray();
            panel.Children.Add(notes);
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
            panel.Children.Add(AttendeeBlock(_detail.Attendees, _brushes));
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
    internal static StackPanel AttendeeBlock(IReadOnlyList<EventAttendee> attendees, ThemeBrushes brushes)
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
            block.Children.Add(AttendeeRow(attendee, brushes));
        }
        return block;
    }

    // One attendee: name (or address), the address + "Organiser" beneath it, and how they answered.
    // Every string is attacker-controlled, the core has already stripped control characters and
    // bidi overrides, and a TextBlock renders text, so there is nothing further to escape.
    private static Grid AttendeeRow(EventAttendee attendee, ThemeBrushes brushes)
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
        var subtitle = AttendeeSummary.Subtitle(
            attendee, L10n.EventAttendeeOrganizer(), L10n.EventAttendeeOptional());
        if (subtitle.Length > 0)
        {
            who.Children.Add(SecondaryOf(subtitle, brushes));
        }
        row.Children.Add(who);

        var answer = SecondaryOf(CalendarEventText.AttendeeResponse(attendee.Response), brushes);
        answer.VerticalAlignment = VerticalAlignment.Top;
        Grid.SetColumn(answer, 1);
        row.Children.Add(answer);
        return row;
    }

    // `automationId` names the VALUE, not the row: a label is localised, so a test that finds this
    // row by its caption passes or fails by the language the developer's machine is in. `linked`
    // makes the value's web and mail addresses clickable, for sender text such as the notes.
    private static StackPanel DetailRow(string label, string value, string? automationId = null, bool linked = false)
    {
        var text = new TextBlock { TextWrapping = TextWrapping.Wrap };
        if (linked)
        {
            LinkedTextBlock.Fill(text, value);
        }
        else
        {
            text.Text = value;
        }
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

    private TextBlock Secondary(string text) => SecondaryOf(text, _brushes);

    // Static twin, for the rows the editor dialog hosts too: they draw in the theme of whichever
    // dialog is showing them, so the brushes come from the caller rather than from a field here.
    private static TextBlock SecondaryOf(string text, ThemeBrushes brushes) => new()
    {
        Text = text,
        Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        Foreground = brushes.Of(ThemePalette.SecondaryText),
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
