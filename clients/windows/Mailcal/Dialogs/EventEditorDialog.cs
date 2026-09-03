// The event editor, one dialog for both create and edit, the Windows twin of Android's
// EventEditorScreen and Apple's EventEditorView. Title, an all-day toggle, start/end date+time
// pickers, a calendar row (a writable-calendar picker on create, display-only on edit), notes, a
// location field (settable on create and edit alike), and reminder + recurrence rows shown but not
// yet editable (v1).
//
// Every decision, validity, which fields are frozen on edit, the wall-clock-vs-UTC create form, the
// all-day inclusive↔exclusive conversion, lives in the pure EventEditorState (Calendar/, unit-tested
// in Mailcal.Tests). This file is the WinUI chrome that binds to it: it mutates the shared state and,
// on Save, leaves it to the caller (CalendarView) to dispatch CreateEvent / UpdateEvent from
// state.CreateArgs() / state.UpdateArgs(). Built imperatively in code-behind, like CalendarManagerDialog,
// so there is no client-side state to drift and the tree is deterministic.
using System;
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The create/edit event form. Read the mutated <see cref="State"/> after a Primary result.</summary>
public sealed class EventEditorDialog : ContentDialog
{
    private readonly MailboxModel _model;
    private readonly EventEditorState _state;
    private readonly CalendarRow[] _writable;

    // Held so the dialog can put the caret in it once it is on screen.
    private TextBox _title = new();

    private readonly TimePicker _startTime = new() { ClockIdentifier = "24HourClock" };
    private readonly TimePicker _endTime = new() { ClockIdentifier = "24HourClock" };
    private readonly DatePicker _startDate = new();
    private readonly DatePicker _endDate = new();
    private readonly Button _calendarButton = new() { HorizontalAlignment = HorizontalAlignment.Stretch };

    /// <summary>The editor state, mutated in place, the caller reads its args after a Primary result.</summary>
    internal EventEditorState State => _state;

    /// <summary>Builds the editor over a create- or edit-seeded <paramref name="state"/>.</summary>
    internal EventEditorDialog(MailboxModel model, EventEditorState state)
    {
        _model = model;
        _state = state;
        // The picker only ever offers calendars a new/edited event could actually land in.
        _writable = model.Calendars().Where(c => c.CanWrite).ToArray();

        Title = state.IsEditing ? L10n.EventEditTitle() : L10n.EventNewTitle();
        PrimaryButtonText = L10n.ActionSave();
        CloseButtonText = L10n.ActionCancel();
        DefaultButton = ContentDialogButton.Primary;
        IsPrimaryButtonEnabled = state.IsValid;

        // The default ContentDialog max-width (548) is narrower than the Start/End row needs, and would
        // clip the time picker even with a wide form. Lift it so the form width below is what decides.
        Resources["ContentDialogMaxWidth"] = FormWidth + 120d;

        Content = new ScrollViewer
        {
            Content = BuildForm(),
            MinWidth = FormWidth + 40,
            MaxHeight = 520,
            // A tab stop only while EDITING, and that is what keeps the caret out of the title
            // there (§11). A ContentDialog focuses the first focusable control in its content and
            // keeps returning to it, so the fix is to BE that control rather than to fight it: the
            // form scroller sits ahead of the title, so the dialog's own choice is a container and
            // not a text field, arrow keys scroll the form, and the title is still one Tab away.
            //
            // Not on create, where the dialog's choice and the rule agree: the title is first and
            // is where the caret belongs.
            IsTabStop = state.IsEditing,
        };

        // The caret opens where the work starts, the empty title on a new event, the same rule the
        // composer's To follows (docs/calendar.md, docs/contacts.md §4). Not on edit: the event
        // already has a title, and the dates are usually what the user came to change, which the
        // scroller's tab stop above takes care of.
        //
        // On Opened, not in this constructor: a dialog's content has no focus to take until it is
        // shown, and the call is dropped without complaining.
        if (!state.IsEditing)
        {
            Opened += (_, _) => _title.Focus(FocusState.Programmatic);
        }
    }

    // Wide enough that the Start/End row's date picker (month/day/year) and time picker sit side by
    // side without the time picker spilling past the dialog's edge, the row is the widest thing here.
    private const double FormWidth = 560;

    private StackPanel BuildForm()
    {
        var panel = new StackPanel { Spacing = 12, Width = FormWidth };

        _title = new TextBox { Header = L10n.EventTitleLabel(), Text = _state.Title };
        // Named for the UI suite: the header is localised, so Name is not something a test can
        // match on, and the two focus rules above are exactly what it has to read.
        AutomationProperties.SetAutomationId(_title, "EventTitleBox");
        _title.TextChanged += (_, _) =>
        {
            _state.Title = _title.Text;
            Revalidate();
        };
        panel.Children.Add(_title);

        // All-day is set at create and frozen on edit (the patcher refuses a form change), so the toggle
        // is disabled while editing, and toggling it hides the time-of-day pickers.
        var allDay = new ToggleSwitch
        {
            Header = L10n.CalendarAllDay(),
            IsOn = _state.AllDay,
            IsEnabled = _state.CanEditForm,
        };
        allDay.Toggled += (_, _) =>
        {
            _state.AllDay = allDay.IsOn;
            _startTime.Visibility = _endTime.Visibility = allDay.IsOn ? Visibility.Collapsed : Visibility.Visible;
            Revalidate();
        };
        panel.Children.Add(allDay);

        // Start/End: a date picker, and a time picker hidden for an all-day event.
        SeedDate(_startDate, _startTime, _state.Start);
        SeedDate(_endDate, _endTime, _state.End);
        _startTime.Visibility = _endTime.Visibility = _state.AllDay ? Visibility.Collapsed : Visibility.Visible;

        _startDate.DateChanged += (_, _) =>
        {
            // Drag the end along when the start passes it, so the interval never inverts.
            if (_endDate.Date < _startDate.Date)
            {
                _endDate.Date = _startDate.Date;
            }
            ReadTimes();
        };
        _endDate.DateChanged += (_, _) => ReadTimes();
        _startTime.TimeChanged += (_, _) => ReadTimes();
        _endTime.TimeChanged += (_, _) => ReadTimes();

        panel.Children.Add(TimeRow(L10n.EventStart(), _startDate, _startTime));
        panel.Children.Add(TimeRow(L10n.EventEnd(), _endDate, _endTime));

        // Calendar, a picker on create, display-only on edit (no cross-calendar move yet).
        RefreshCalendarButton();
        _calendarButton.IsEnabled = _state.CanEditForm;
        if (_state.CanEditForm)
        {
            _calendarButton.Flyout = CalendarFlyout();
        }
        panel.Children.Add(new StackPanel
        {
            Spacing = 4,
            Children =
            {
                new TextBlock { Text = L10n.EventCalendar(), Style = Caption() },
                _calendarButton,
            },
        });

        // Location: settable on create and edit alike, the engine's create draft carries it.
        var location = new TextBox { Header = L10n.EventLocation(), Text = _state.Location };
        location.TextChanged += (_, _) => _state.Location = location.Text;
        panel.Children.Add(location);

        var notes = new TextBox
        {
            Header = L10n.EventNotes(),
            Text = _state.Notes,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
        };
        notes.TextChanged += (_, _) => _state.Notes = notes.Text;
        panel.Children.Add(notes);

        // Reminder: shown, not yet editable. On create the state has none, so it reads "None".
        panel.Children.Add(ReadOnlyRow(L10n.EventReminder(), CalendarEventText.Reminder(_state.Editing?.ReminderMinutes)));

        // The repeat is a set of controls when the core handed over a draft, and the sentence it
        // already decided when it did not.
        if (_state.CanEditRepeat)
        {
            panel.Children.Add(EventRepeatEditor.Build(_state, CaptionText));
        }
        else
        {
            panel.Children.Add(ReadOnlyRow(
                L10n.EventRepeat(),
                EventRepeatText.Summary(
                    _state.Editing?.RepeatSummary,
                    _state.Editing?.IsRecurring ?? false,
                    CultureInfo.CurrentCulture),
                "EventRepeatValue"));
            panel.Children.Add(CaptionText(L10n.EventRepeatLocked()));
        }

        // Only when the answer is settled. An editor opened on one occurrence asks at Save which
        // occurrences were meant, so stating the answer up here would tell the user something the
        // next dialog contradicts.
        if (_state.Editing?.IsRecurring == true && !_state.AsksAboutTheSeries
            && string.IsNullOrEmpty(_state.Editing?.Occurrence))
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.EventSeriesNote(),
                Style = Caption(),
                Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
                TextWrapping = TextWrapping.Wrap,
            });
        }

        // Attendees: shown so an edit is not made blind to who is coming, and stated to be read-only
        // rather than offered as a field that would quietly drop the change, editing them means
        // sending iTIP updates, which is its own feature.
        var attendees = _state.Editing?.Attendees;
        if (attendees is { Count: > 0 })
        {
            panel.Children.Add(EventDetailDialog.AttendeeBlock(attendees));
            panel.Children.Add(new TextBlock
            {
                Text = L10n.EventAttendeesReadOnly(),
                Style = Caption(),
                Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
                TextWrapping = TextWrapping.Wrap,
            });
        }

        return panel;
    }

    // Folds the two pickers back into the state's wall clock, plain numbers, never converted to UTC,
    // so a created event reads back the same clock it was typed in (its zone is tracked separately).
    private void ReadTimes()
    {
        _state.Start = WallClock(_startDate.Date, _startTime.Time);
        _state.End = WallClock(_endDate.Date, _endTime.Time);
        Revalidate();
    }

    private void Revalidate() => IsPrimaryButtonEnabled = _state.IsValid;

    // A labelled date (+ time, hidden when all-day) row.
    private static StackPanel TimeRow(string label, DatePicker date, TimePicker time) => new()
    {
        Spacing = 4,
        Children =
        {
            new TextBlock { Text = label, Style = Caption() },
            new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 8,
                Children = { date, time },
            },
        },
    };

    // `automationId` names the VALUE, not the row: a label is localised, so a test that finds this
    // row by its caption passes or fails by the language the developer's machine is in.
    /// <summary>Secondary explanatory text, as every note under a field in this dialog draws it.</summary>
    internal static UIElement CaptionText(string text) => new TextBlock
    {
        Text = text,
        Style = Caption(),
        Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
        TextWrapping = TextWrapping.Wrap,
    };

    private static StackPanel ReadOnlyRow(string label, string value, string? automationId = null)
    {
        var text = new TextBlock { Text = value };
        if (automationId is not null)
        {
            AutomationProperties.SetAutomationId(text, automationId);
        }
        return new StackPanel
        {
            Spacing = 2,
            Children = { new TextBlock { Text = label, Style = Caption() }, text },
        };
    }

    // The colour dot + name shown on the calendar button, refreshed after a pick.
    private void RefreshCalendarButton()
    {
        var choice = _state.Calendar;
        var row = _writable.FirstOrDefault(c => c.Account == choice?.Account && c.Id == choice?.Id);
        var name = row?.Name ?? choice?.Name ?? string.Empty;

        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
        if (row is not null)
        {
            content.Children.Add(new Border
            {
                Width = 14,
                Height = 14,
                CornerRadius = new CornerRadius(7),
                VerticalAlignment = VerticalAlignment.Center,
                Background = new SolidColorBrush(CalendarColors.Parse(
                    (ActualTheme == ElementTheme.Dark ? row.Color.Dark : row.Color.Light).Background)),
            });
        }
        content.Children.Add(new TextBlock
        {
            Text = name,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        _calendarButton.Content = content;
    }

    // The writable-calendar picker: grouped by account, since a calendar id is unique only within one.
    private Flyout CalendarFlyout()
    {
        var flyout = new Flyout();
        var list = new StackPanel { Spacing = 6, MinWidth = 260 };
        foreach (var group in _writable.GroupBy(c => c.Account))
        {
            var account = _model.Accounts.FirstOrDefault(a => a.Id == group.Key);
            list.Children.Add(new TextBlock
            {
                Text = account?.Email ?? group.Key,
                FontWeight = FontWeights.SemiBold,
                Style = Caption(),
            });
            foreach (var row in group)
            {
                list.Children.Add(CalendarChoiceButton(flyout, row));
            }
        }
        flyout.Content = new ScrollViewer { Content = list, MaxHeight = 320 };
        return flyout;
    }

    private Button CalendarChoiceButton(Flyout flyout, CalendarRow row)
    {
        var dot = new Border
        {
            Width = 14,
            Height = 14,
            CornerRadius = new CornerRadius(7),
            VerticalAlignment = VerticalAlignment.Center,
            Background = new SolidColorBrush(CalendarColors.Parse(
                (ActualTheme == ElementTheme.Dark ? row.Color.Dark : row.Color.Light).Background)),
        };
        var button = new Button
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 10,
                Children = { dot, new TextBlock { Text = row.Name, VerticalAlignment = VerticalAlignment.Center } },
            },
        };
        button.Click += (_, _) =>
        {
            _state.Calendar = new CalendarChoice(row.Account, row.Id, row.Name);
            RefreshCalendarButton();
            flyout.Hide();
        };
        return button;
    }

    private static Style Caption() => (Style)Application.Current.Resources["CaptionTextBlockStyle"];

    // ---- Wall-clock <-> pickers (numbers only; the zone is tracked apart in the state) --------------

    // The DatePicker wants a DateTimeOffset. Build it at *local* midnight of the wall-clock date, so the
    // date the user sees is the date the state holds, the offset only bites if the value were ever
    // converted to UTC, which it never is (the wall clock stays plain numbers; its zone is tracked apart).
    private static void SeedDate(DatePicker date, TimePicker time, DateTime wall)
    {
        date.Date = new DateTimeOffset(
            new DateTime(wall.Year, wall.Month, wall.Day, 0, 0, 0, DateTimeKind.Local));
        time.Time = wall.TimeOfDay;
    }

    private static DateTime WallClock(DateTimeOffset date, TimeSpan time) =>
        new DateTime(date.Year, date.Month, date.Day, 0, 0, 0, DateTimeKind.Unspecified) + time;
}
