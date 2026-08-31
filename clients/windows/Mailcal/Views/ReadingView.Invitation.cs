// The reading pane's invitation slot: the card when the open message carries one, and nothing at all
// otherwise.
//
// The decision is entirely the core's, its two-condition RSVP gate (a scheduling METHOD *and* an
// ATTENDEE matching one of this account's own addresses, docs/invitations.md), so a published .ics
// produces no card here and keeps its attachment chip instead. A stale snapshot for a
// previously-opened message is already filtered out upstream, in Render().
//
// Split from ReadingView.xaml.cs for the 500-line cap, along the seam that file already uses: that
// one owns which of loading / html / plain / empty / error is showing, this owns one banner.
using System.Globalization;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ReadingView
{
    /// <summary>Built on first use, most messages are not invitations, so most panes never need it.</summary>
    private InvitationCardView? _invitation;

    // Draw (or clear) the card for the body snapshot now showing. Called from Render(), so it moves
    // with the rest of the pane: opening another message, or a snapshot arriving, rebuilds it.
    private void SetInvitation(ReadingBody? body)
    {
        if (_model is null || body?.Invitation is not { } card)
        {
            InvitationHost.Visibility = Visibility.Collapsed;
            InvitationHost.Child = null;
            _invitation = null;
            return;
        }
        if (_invitation is null)
        {
            _invitation = new InvitationCardView
            {
                // The message this card belongs to, resolved at click time rather than captured: the
                // pane advances through messages and the handler outlives any one of them. The core
                // resolves everything else, which address answers, which event it lands on, from
                // the message alone (docs/invitations.md §4).
                Respond = (response, comment, notify, replySubject) =>
                {
                    if (_model?.OpenedMessage is { } opened)
                    {
                        _model.RespondToInvitation(
                            opened.Account, opened.Key, response, comment, notify, replySubject);
                    }
                },
            };
            InvitationHost.Child = _invitation;
        }
        _invitation.Apply(card, _model.ActiveZone, Use24Hour(), CultureInfo.CurrentCulture, _model.CalendarWrite);
        InvitationHost.Visibility = Visibility.Visible;
    }

    // The calendar write settling right now, reported without rebuilding the card, a rebuild would
    // take a half-typed note to the organiser away mid-sentence. Only the respond row moves.
    private void OnCalendarWriteChanged()
    {
        if (_model is not null)
        {
            _invitation?.SetWriteStatus(_model.CalendarWrite);
        }
    }

    // The app's 12/24-hour setting, not the culture's default: mail and calendar must not disagree
    // with each other about whether it is 14:05 or 2:05 PM (docs/timestamps.md). Falls back to the
    // 24-hour clock the rest of the client defaults to when the core has not answered yet.
    private bool Use24Hour() =>
        _model?.CalendarDisplaySettings() is not { } display
        || display.TimeFormat == TimeFormat.TwentyFourHour;
}
