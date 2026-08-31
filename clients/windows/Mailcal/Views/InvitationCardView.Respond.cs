// Accept / Maybe / Decline, and the two controls that ride beside them on the transports that have
// them.
//
// Its own partial because it is the only part of the card that *writes*, and because everything in it
// is conditional on what the account can actually do, the card itself stays a straight render of what
// the core computed.
//
// # Three gates, none of them a disabled button
//
//   - `canRespond`, the account's calendar cannot RSVP at all. The buttons are then *absent* and a
//     sentence says why. A greyed-out Accept invites the user to try, wonder, and try again;
//     "this account can't send a response" ends it.
//   - `canComment`, the transport has nowhere to put a note (CalDAV, JMAP). The field is absent,
//     because the core **refuses** a note it cannot carry rather than dropping it: an offered field
//     would not merely lose the text, it would lose the whole answer.
//   - `canChooseNotify`, the server sends the reply the moment the status changes and no client can
//     stop it. The toggle is absent for the same reason: one that emails the organiser anyway is
//     worse than none.
//
// On both harness accounts, and on any CalDAV or JMAP account, this is three buttons and nothing
// else. That is the truth of the transport, not a missing feature.
using Allodia.Mailcal.Calendar;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

internal sealed partial class InvitationCardView
{
    /// <summary>The note for the organiser, present only where the transport carries one.</summary>
    private TextBox? _comment;

    /// <summary>The "email the organiser" tick, present only where the server can be told not to.</summary>
    private CheckBox? _notify;

    /// <summary>The three answers, so a settling write can take them out of reach together.</summary>
    private readonly List<Button> _answers = new();

    /// <summary>What happened to the answer, blank unless there is something to say.</summary>
    private TextBlock? _writeLine;

    /// <summary>
    /// Drops every reference to the previous card's respond controls.
    /// </summary>
    /// <remarks>
    /// Called from <c>Apply</c> rather than only from <see cref="RespondRow"/>, because a card that
    /// builds no respond row at all, a cancellation, or an informational notice, would otherwise
    /// leave the *previous* invitation's buttons here, and the next write status would enable and
    /// re-label controls that are no longer on screen.
    /// </remarks>
    private void ResetRespondRow()
    {
        _comment = null;
        _notify = null;
        _answers.Clear();
        _writeLine = null;
    }

    private UIElement RespondRow(InvitationCard card, CalendarWriteStatus status)
    {
        var stack = new StackPanel { Spacing = 6, Margin = new Thickness(0, 4, 0, 0) };
        if (!card.CanRespond)
        {
            // Absent with an explanation, never present and disabled: a button that appears to work
            // but tells nobody is worse than no button.
            stack.Children.Add(Caption(L10n.InvitationCannotRespond()));
            return stack;
        }

        if (card.CanComment)
        {
            _comment = new TextBox
            {
                PlaceholderText = L10n.InvitationMessageToOrganizer(),
                AcceptsReturn = true,
                TextWrapping = TextWrapping.Wrap,
                MaxHeight = 72,
            };
            AutomationProperties.SetName(_comment, L10n.InvitationMessageToOrganizer());
            stack.Children.Add(_comment);
        }
        if (card.CanChooseNotify)
        {
            // Ticked by default, mirroring RFC 5546: an invitation asks for a reply, so answering
            // sends one. The user has to say otherwise.
            _notify = new CheckBox { Content = L10n.InvitationNotifyOrganizer(), IsChecked = true };
            stack.Children.Add(_notify);
        }

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        buttons.Children.Add(Answer(
            card, InvitationResponse.Accept,
            L10n.InvitationAccept(), L10n.A11yInvitationAccept(), accent: true));
        buttons.Children.Add(Answer(
            card, InvitationResponse.Tentative,
            L10n.InvitationTentative(), L10n.A11yInvitationTentative(), accent: false));
        buttons.Children.Add(Answer(
            card, InvitationResponse.Decline,
            L10n.InvitationDecline(), L10n.A11yInvitationDecline(), accent: false));
        stack.Children.Add(buttons);

        _writeLine = new TextBlock
        {
            Style = Res("CaptionTextBlockStyle"),
            TextWrapping = TextWrapping.Wrap,
            Visibility = Visibility.Collapsed,
        };
        stack.Children.Add(_writeLine);
        ApplyWriteStatus(status);
        return stack;
    }

    /// <summary>
    /// One answer.
    /// </summary>
    /// <remarks>
    /// The label is the visible word ("Accept"); the automation name says what it acts on, because
    /// three bare verbs read out of context tell a screen-reader user nothing about which invitation
    /// they belong to.
    /// </remarks>
    private Button Answer(
        InvitationCard card, InvitationResponse response, string title, string spoken, bool accent)
    {
        var button = new Button { Content = title };
        if (accent)
        {
            button.Style = Res("AccentButtonStyle");
        }
        AutomationProperties.SetName(button, spoken);
        button.Click += (_, _) => Respond?.Invoke(
            response,
            // Only where the transport carries one: sending a note it cannot carry fails the whole
            // answer rather than quietly losing the text.
            card.CanComment ? _comment?.Text ?? string.Empty : null,
            !card.CanChooseNotify || _notify?.IsChecked == true,
            // The subject for the reply the core may have to email itself. Composed here because
            // the summary and the chosen answer are both in hand, and because the core has no
            // locale to compose it with.
            InvitationText.ReplySubject(response, card.Summary));
        _answers.Add(button);
        return button;
    }

    /// <summary>
    /// Reports the write currently settling, without rebuilding the card.
    /// </summary>
    /// <remarks>
    /// <c>Saving</c> and <c>Failed</c> are the two the user must see. A reply the organiser never
    /// received, reported as sent, is the failure this whole feature exists to prevent, so a failure
    /// says so in words rather than leaving the old answer on screen looking settled. <c>Saved</c>
    /// says nothing on purpose: by then the card has been rebuilt from the calendar and already shows
    /// the new answer, so a second line would be noise.
    /// </remarks>
    internal void SetWriteStatus(CalendarWriteStatus status) => ApplyWriteStatus(status);

    private void ApplyWriteStatus(CalendarWriteStatus status)
    {
        var settling = status == CalendarWriteStatus.Saving;
        foreach (var button in _answers)
        {
            button.IsEnabled = !settling;
        }
        if (_comment is not null)
        {
            _comment.IsEnabled = !settling;
        }
        if (_notify is not null)
        {
            _notify.IsEnabled = !settling;
        }
        if (_writeLine is null)
        {
            return;
        }
        var line = InvitationText.Write(status);
        _writeLine.Text = line ?? string.Empty;
        _writeLine.Visibility = line is null ? Visibility.Collapsed : Visibility.Visible;
        _writeLine.Foreground = ThemeBrush(
            InvitationFormat.Write(status) == WriteLine.Failed
                ? "SystemFillColorCriticalBrush"
                : "TextFillColorSecondaryBrush");
    }
}
