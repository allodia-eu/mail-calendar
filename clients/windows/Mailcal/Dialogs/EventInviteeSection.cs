// The meeting's invitee list inside the event editor: who it invites, and whether each one is
// required or optional, as the organiser edits it. The Windows twin of Android's EventInvitees.kt.
//
// Offered only where the core says a meeting may be organised (docs/calendar.md, "Organising a
// meeting"): on a create, a calendar whose server sends the invitations; on an edit, a meeting this
// account organises. Somebody else's meeting shows its list read-only, with the sentence saying so,
// and a create on a calendar that cannot invite says why nobody can be added. The state is
// InviteeRoster's; this file only draws it.
using System;
using System.Linq;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The editor's attendee block, editable where this account may organise the meeting.</summary>
internal sealed class EventInviteeSection
{
    private readonly MailboxModel _model;
    private readonly EventEditorState _state;
    private readonly ThemeBrushes _brushes;
    private readonly Func<string, UIElement> _caption;
    private readonly StackPanel _panel = new() { Spacing = 4 };
    private readonly StackPanel _rows = new() { Spacing = 2 };
    private readonly TextBlock _refusal = new() { TextWrapping = TextWrapping.Wrap };

    // The heading over a list with no fixed rows above it, a create's: drawn only once somebody is
    // on the list, since a heading over nothing says "we looked and found none".
    private readonly TextBlock _heading = new() { Margin = new Thickness(0, 10, 0, 0) };
    private RecipientField? _field;

    // Who the rows were last drawn for, see Refresh.
    private string? _drawn;

    /// <param name="caption">The editor's own secondary-text style, for the notes under the list.</param>
    internal EventInviteeSection(
        MailboxModel model, EventEditorState state, ThemeBrushes brushes, Func<string, UIElement> caption)
    {
        _model = model;
        _state = state;
        _brushes = brushes;
        _caption = caption;
        _refusal.Foreground = brushes.Of(ThemePalette.Critical);
        _refusal.Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"];
        AutomationProperties.SetAutomationId(_refusal, "EventInviteeRefusal");
        _heading.Text = L10n.EventAttendees();
        _heading.Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"];
        AutomationProperties.SetAutomationId(_heading, "EventAttendeesHeading");
        state.Roster.Changed += Refresh;
        Rebuild();
    }

    /// <summary>
    /// Stops following the roster. The editor reopens over the same state after a question it
    /// raised is backed out of, so a closed editor's section must not go on redrawing.
    /// </summary>
    internal void Detach() => _state.Roster.Changed -= Refresh;

    /// <summary>The section, rebuilt in place when the picked calendar changes.</summary>
    internal UIElement View => _panel;

    /// <summary>
    /// Adds what is still typed, as a Save does first; <c>false</c> when it cannot be invited, and
    /// the field keeps it beside the reason.
    /// </summary>
    internal bool TakeTyped()
    {
        if (!_state.CanInvite)
        {
            return true;
        }
        var added = _state.Roster.TakeTyped();
        if (!added)
        {
            _field?.FocusInput();
        }
        return added;
    }

    /// <summary>Redraws the whole section: whether it is offered moves with the calendar picked.</summary>
    internal void Rebuild()
    {
        _panel.Children.Clear();
        _field = null;
        _drawn = null;
        var editing = _state.Editing;
        var attendees = editing?.Attendees ?? [];
        if (!_state.CanInvite)
        {
            if (editing is null)
            {
                var unavailable = _caption(L10n.EventInviteesUnavailable());
                AutomationProperties.SetAutomationId(unavailable, "EventInviteesUnavailable");
                _panel.Children.Add(unavailable);
            }
            else if (attendees.Count > 0)
            {
                // Shown so an edit is not made blind to who is coming, and stated to be read-only
                // rather than offered as a field that would quietly drop the change.
                _panel.Children.Add(EventDetailDialog.AttendeeBlock(attendees, _brushes));
                _panel.Children.Add(_caption(L10n.EventAttendeesReadOnly()));
            }
            return;
        }

        // The rows no control here may change: the organiser, and a role the editor cannot state.
        var fixedRows = attendees.Where(attendee => attendee.IsOrganizer || attendee.Role is null).ToList();
        _panel.Children.Add(fixedRows.Count > 0 ? EventDetailDialog.AttendeeBlock(fixedRows, _brushes) : _heading);
        _panel.Children.Add(_rows);
        _panel.Children.Add(FieldRow());
        _panel.Children.Add(_refusal);
        _panel.Children.Add(_caption(L10n.EventInviteesSendNote()));
        // The calendar picked may have changed who organises.
        _state.Roster.Recheck();
        Refresh();
    }

    private Grid FieldRow()
    {
        var field = new RecipientField
        {
            Placeholder = L10n.EventInviteeField(),
            InputAutomationId = "EventInviteeField",
            SuggestionsFor = _model.RecipientSuggestionsAsync,
        };
        // A finished address (a comma, or an accepted suggestion) moves out of the field into the
        // list, and Refresh writes back what is left.
        field.RecipientsChanged += (_, _) => _state.Roster.FieldChanged(field.Text);
        field.Submitted += (_, _) => TakeTyped();
        _field = field;

        var add = new Button { Content = L10n.EventInviteeAdd(), VerticalAlignment = VerticalAlignment.Top };
        AutomationProperties.SetAutomationId(add, "EventInviteeAdd");
        add.Click += (_, _) => TakeTyped();

        var row = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(field);
        Grid.SetColumn(add, 1);
        row.Children.Add(add);
        return row;
    }

    // The list and the refusal follow the roster. The field is rewritten only when what it holds
    // differs from the roster's token by more than spacing, so a space being typed survives; the
    // rows are redrawn only when who is on them changed, so a ticked box keeps its focus.
    private void Refresh()
    {
        var roster = _state.Roster;
        if (_field is not null && _field.Text.Trim() != roster.Field)
        {
            _field.Text = roster.Field;
        }
        var refusal = Refusal(roster.Refusal);
        _refusal.Text = refusal ?? string.Empty;
        _refusal.Visibility = refusal is null ? Visibility.Collapsed : Visibility.Visible;
        _heading.Visibility = roster.Invitees.Count > 0 ? Visibility.Visible : Visibility.Collapsed;

        var drawn = string.Join(' ', roster.Invitees.Select(invitee => invitee.Email))
            + (roster.KeepsLastInvitee ? " !" : string.Empty);
        if (drawn == _drawn)
        {
            return;
        }
        _drawn = drawn;
        _rows.Children.Clear();
        var attendees = _state.Editing?.Attendees ?? [];
        for (var index = 0; index < roster.Invitees.Count; index++)
        {
            var invitee = roster.Invitees[index];
            var stored = attendees.FirstOrDefault(
                attendee => string.Equals(attendee.Email, invitee.Email, StringComparison.OrdinalIgnoreCase));
            _rows.Children.Add(InviteeRow(index, invitee, stored, removable: !roster.KeepsLastInvitee));
        }
    }

    private Grid InviteeRow(int index, MeetingInvitee invitee, EventAttendee? stored, bool removable)
    {
        var shown = string.IsNullOrEmpty(invitee.Name) ? invitee.Email : invitee.Name;
        var row = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 6, 0, 0) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var who = new StackPanel { Spacing = 1, VerticalAlignment = VerticalAlignment.Center };
        who.Children.Add(new TextBlock { Text = shown, TextTrimming = TextTrimming.CharacterEllipsis });
        if (shown != invitee.Email)
        {
            who.Children.Add(Secondary(invitee.Email));
        }
        if (stored is not null)
        {
            who.Children.Add(Secondary(CalendarEventText.AttendeeResponse(stored.Response)));
        }
        row.Children.Add(who);

        var optional = new CheckBox
        {
            Content = L10n.EventAttendeeOptional(),
            IsChecked = invitee.Role == MeetingInviteeRole.Optional,
            MinWidth = 0,
            VerticalAlignment = VerticalAlignment.Center,
        };
        // The person as help text, so the boxes are told apart by a screen reader and a test alike.
        AutomationProperties.SetHelpText(optional, shown);
        AutomationProperties.SetAutomationId(optional, $"EventInviteeOptional:{invitee.Email}");
        optional.Checked += (_, _) => _state.Roster.SetOptional(index, true);
        optional.Unchecked += (_, _) => _state.Roster.SetOptional(index, false);
        Grid.SetColumn(optional, 1);
        row.Children.Add(optional);

        // E711 Cancel: the "Remove an invitee" row of docs/icons.md. Named for the person, so the
        // fourth of four identical buttons is still distinguishable; and, when the core would refuse
        // it, why not.
        var label = L10n.EventInviteeRemove(shown);
        var remove = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = 12 },
            IsEnabled = removable,
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetName(remove, removable ? label : $"{label}. {L10n.EventInviteeLast()}");
        AutomationProperties.SetAutomationId(remove, $"EventInviteeRemove:{invitee.Email}");
        ToolTipService.SetToolTip(remove, removable ? label : L10n.EventInviteeLast());
        remove.Click += (_, _) =>
        {
            _state.Roster.Remove(index);
            // The button went with its row; the field is where the next change starts.
            _field?.FocusInput();
        };
        Grid.SetColumn(remove, 2);
        row.Children.Add(remove);
        return row;
    }

    private TextBlock Secondary(string text) => new()
    {
        Text = text,
        Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        Foreground = _brushes.Of(ThemePalette.SecondaryText),
        TextTrimming = TextTrimming.CharacterEllipsis,
    };

    /// <summary>What the field says when an address cannot be added; <c>null</c> for one that can.</summary>
    private static string? Refusal(InviteeCheck? check) => check switch
    {
        InviteeCheck.NotAnAddress => L10n.EventInviteeNotAnAddress(),
        InviteeCheck.AlreadyInvited => L10n.EventInviteeAlreadyInvited(),
        InviteeCheck.Organiser => L10n.EventInviteeOrganiser(),
        _ => null,
    };
}
