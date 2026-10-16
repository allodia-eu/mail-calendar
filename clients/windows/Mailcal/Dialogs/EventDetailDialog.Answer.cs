// An invitee's answer in an event's detail: what they answered, and Accept, Maybe and Decline to
// change it, with the decisions in Calendar/EventAnswer.cs. The words are the invitation card's, so
// the two places a meeting is answered read the same.
//
// Accept and Maybe go at once and the detail stays open: it re-reads the event when the calendar
// changes and shows the new answer, and no button can be pressed while the write is saving. Decline
// asks first, and WinUI shows one ContentDialog at a time, so the detail closes with Declining set
// and the caller asks, reopening the detail if the user backs out.
//
// The "organiser wasn't told" question about an answer given here is put from here while the detail
// is open. The window's own bar sits under this dialog's overlay, where nobody can reach it until
// the detail is closed. A question left standing from an earlier answer, the invitation card's
// included, stays the window's: it is about another meeting.
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class EventDetailDialog
{
    private readonly InfoBar _replyBar = new()
    {
        IsClosable = false,
        Severity = InfoBarSeverity.Warning,
        Margin = new Thickness(0, 0, 0, 8),
    };

    private readonly CheckBox _rememberReply = new();

    // The answer buttons on screen, by the answer each sends.
    private readonly Dictionary<InvitationResponse, Button> _answerButtons = [];

    // Answer buttons a redraw took out of the tree: focus leaving one is not focus entering.
    private readonly HashSet<Button> _retiredAnswers = [];

    // Whether this detail has sent an answer, which is what makes a reply question its own.
    private bool _answered;

    // Whether the answer was pressed from the keyboard, so the redrawn row takes the focus back.
    private bool _answeredFromKeyboard;

    /// <summary>Whether the user pressed Decline: the caller asks "Decline this meeting?" once this
    /// dialog has closed, and sends the answer only on a yes.</summary>
    internal bool Declining { get; private set; }

    // The reply question's bar, built once; its message and visibility follow the core.
    private void BuildReplyBar()
    {
        _replyBar.Title = L10n.InvitationReplyUndeliveredTitle();
        _rememberReply.Content = L10n.InvitationReplyUndeliveredRemember();
        var send = new Button
        {
            Content = L10n.InvitationReplyUndeliveredSend(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        };
        send.Click += (_, _) => AnswerReply(send: true);
        var dismiss = new Button { Content = L10n.InvitationReplyUndeliveredDismiss() };
        dismiss.Click += (_, _) => AnswerReply(send: false);
        _replyBar.Content = new StackPanel
        {
            Spacing = 8,
            Margin = new Thickness(0, 0, 0, 12),
            Children =
            {
                _rememberReply,
                new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Children = { send, dismiss } },
            },
        };
        AutomationProperties.SetAutomationId(_replyBar, "EventReplyPrompt");
        ShowReplyPrompt();
    }

    private void ShowReplyPrompt()
    {
        var shown = _answered && _model.HasReplyPrompt;
        _replyBar.Message = _model.ReplyPromptText;
        _replyBar.IsOpen = shown;
        _replyBar.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;
    }

    // The tick applies to whichever way the user went, and is reset for the next question, as the
    // window's bar does.
    private void AnswerReply(bool send)
    {
        var remember = _rememberReply.IsChecked == true;
        _rememberReply.IsChecked = false;
        var prompt = _model.ReplyPrompt;
        _model.AnswerReplyPrompt(
            send,
            remember,
            prompt is null ? null : InvitationText.ReplySubject(prompt.Response, prompt.Summary));
    }

    // The model's signals, while the dialog is open: a calendar change re-reads the event, a write
    // status holds or frees the buttons, and the reply question comes and goes. The version moves
    // on every calendar refresh, so the body is redrawn only when what it shows changed.
    private void OnModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(MailboxModel.CalendarVersion):
                if (_model.EventDetail(_detail.Account, _detail.Key, _detail.OccurrenceStart) is { } fresh)
                {
                    var changed = Shown(fresh) != Shown(_detail);
                    _detail = fresh;
                    if (changed)
                    {
                        Rebuild();
                    }
                }
                else
                {
                    // The event left the calendar under the open detail; there is nothing to show.
                    Hide();
                }
                break;
            case nameof(MailboxModel.CalendarWrite):
                HoldAnswers();
                break;
            case nameof(MailboxModel.ReplyPrompt):
                ShowReplyPrompt();
                break;
        }
    }

    /// <summary>The answer row, or <c>null</c> where the core offers none.</summary>
    private StackPanel? AnswerBlock()
    {
        if (EventAnswer.Of(_detail, _model.CalendarWrite) is not { } answer)
        {
            return null;
        }
        var block = new StackPanel { Spacing = 6, Margin = new Thickness(0, 10, 0, 0) };
        var line = new TextBlock
        {
            Text = InvitationText.Response(answer.Current),
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
            Foreground = _brushes.Of(ThemePalette.AccentText),
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetAutomationId(line, "EventAnswerLine");
        block.Children.Add(line);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        foreach (var response in EventAnswer.Choices)
        {
            buttons.Children.Add(AnswerButton(answer, response));
        }
        block.Children.Add(buttons);
        return block;
    }

    // Holds the answers while a write is saving and frees them after, in place: redrawing them
    // would take the focus with the button it is on.
    private void HoldAnswers()
    {
        if (EventAnswer.Of(_detail, _model.CalendarWrite) is not { } answer)
        {
            return;
        }
        foreach (var (response, button) in _answerButtons)
        {
            button.IsEnabled = answer.CanPress(response);
        }
        HandFocusToAnswers();
    }

    // What the body draws from a read, so a refresh that changed none of it redraws nothing and
    // leaves the scroll and the focus where they were.
    private static string Shown(EventDetail detail) => string.Join(
        '|',
        detail.Title, detail.Start, detail.End, detail.Timezone, detail.Calendar, detail.Location,
        detail.Notes, detail.ReminderMinutes, detail.IsRecurring, detail.CanWrite, detail.MyAnswer,
        detail.RepeatSummary?.ToString(),
        string.Join(';', detail.Attendees.Select(a => $"{a.Name}|{a.Email}|{a.IsOrganizer}|{a.Role}|{a.Response}")));

    private Button AnswerButton(EventAnswer answer, InvitationResponse response)
    {
        var (title, spoken, id) = response switch
        {
            InvitationResponse.Accept => (L10n.InvitationAccept(), L10n.A11yInvitationAccept(), "EventAnswerAccept"),
            InvitationResponse.Tentative =>
                (L10n.InvitationTentative(), L10n.A11yInvitationTentative(), "EventAnswerTentative"),
            _ => (L10n.InvitationDecline(), L10n.A11yInvitationDecline(), "EventAnswerDecline"),
        };
        // The answer already given is drawn chosen, and is the one button that cannot be pressed:
        // sending it again changes nothing.
        var button = new Button { Content = title, IsEnabled = answer.CanPress(response) };
        if (answer.IsCurrent(response))
        {
            button.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        }
        AutomationProperties.SetName(button, spoken);
        AutomationProperties.SetAutomationId(button, id);
        button.Click += (_, _) =>
        {
            _answeredFromKeyboard = button.FocusState == FocusState.Keyboard;
            Press(response);
        };
        _answerButtons[response] = button;
        return button;
    }

    private void Press(InvitationResponse response)
    {
        if (EventAnswer.Of(_detail, _model.CalendarWrite) is not { } answer)
        {
            return;
        }
        switch (answer.Press(response))
        {
            case AnswerStep.Send:
                _answered = true;
                _model.RespondToEvent(
                    _detail.Account, _detail.Key, response, InvitationText.ReplySubject(response, _detail.Title));
                break;
            case AnswerStep.ConfirmDecline:
                Declining = true;
                Hide();
                break;
        }
    }
}
