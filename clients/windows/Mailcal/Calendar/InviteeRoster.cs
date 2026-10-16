// Who a meeting invites, as the event editor's controls hold it: a plain class, for the reason
// EventEditorState is one, so Mailcal.Tests drives it without constructing a dialog. The twin of
// Android's InviteeRoster.kt.
//
// The list holds the rows an organiser may edit, never the organiser. Addresses arrive through the
// composer's own recipient field, so they complete from the same people index as To does, and each
// finished address moves out of the field into a row of its own. Whether one may be added is the
// core's answer (`check_meeting_invitee`), and what a save sends is worked out from the list by the
// core too (`meeting_invitee_patch`), so this class decides neither.
using System;
using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>
/// What an edited invitee list changes, decided by the core: see <c>meeting_invitee_patch</c>.
/// A delegate for the reason <see cref="RepeatChangeOf"/> is one.
/// </summary>
internal delegate MeetingInviteePatch? InviteePatchOf(
    IReadOnlyList<EventAttendee> attendees, IReadOnlyList<MeetingInvitee> invitees);

/// <summary>Whether a typed address can be invited, decided by the core: see
/// <c>check_meeting_invitee</c>.</summary>
internal delegate InviteeCheck InviteeCheckOf(string typed, IReadOnlyList<string> invited, string organiser);

/// <summary>The invitees an organiser edits, the field they are typed into, and why the last
/// address was refused.</summary>
internal sealed class InviteeRoster
{
    private readonly EditTarget? _editing;
    private readonly InviteeCheckOf _checkOf;
    private readonly Func<string> _organiser;
    private readonly List<MeetingInvitee> _invitees;

    /// <param name="editing">The meeting being edited, or <c>null</c> on a create.</param>
    /// <param name="checkOf">The core's address check.</param>
    /// <param name="organiser">Who organises the meeting; on a create it follows the calendar
    /// picked.</param>
    internal InviteeRoster(EditTarget? editing, InviteeCheckOf checkOf, Func<string> organiser)
    {
        _editing = editing;
        _checkOf = checkOf;
        _organiser = organiser;
        _invitees = editing is { CanEditInvitees: true }
            ? editing.Attendees
                .Where(attendee => !attendee.IsOrganizer && attendee.Role is not null)
                .Select(attendee => new MeetingInvitee(
                    attendee.Name.Length == 0 ? null : attendee.Name, attendee.Email, attendee.Role!.Value))
                .ToList()
            : [];
    }

    /// <summary>Raised when the list, the field or the refusal moved, so the dialog redraws.</summary>
    internal event Action? Changed;

    /// <summary>The invitees, in the order they were added.</summary>
    internal IReadOnlyList<MeetingInvitee> Invitees => _invitees;

    /// <summary>What the field keeps: the token still being typed, after any finished addresses the
    /// core refused, which stay there to be corrected.</summary>
    internal string Field { get; private set; } = string.Empty;

    /// <summary>Why the last address could not be added, or <c>null</c> when nothing was refused.</summary>
    internal InviteeCheck? Refusal { get; private set; }

    /// <summary>
    /// Whether the one invitee left on an existing meeting must stay: a meeting is called off by
    /// deleting it, and a roster change that leaves nobody invited is refused. A row nobody edits
    /// here, such as a chair, still counts as invited.
    /// </summary>
    internal bool KeepsLastInvitee =>
        _invitees.Count == 1
        && _editing is { Attendees.Count: > 0 } editing
        && !editing.Attendees.Any(attendee => !attendee.IsOrganizer && attendee.Role is null);

    /// <summary>The field changed: every finished address in it moves into the list, and what is
    /// left is the token still being typed. From the first finished address the core refuses,
    /// everything stays in the field beside the reason, so it can be corrected rather than
    /// retyped.</summary>
    internal void FieldChanged(string value)
    {
        Take(RecipientTokens.Committed(value), RecipientTokens.CurrentToken(value));
        Changed?.Invoke();
    }

    /// <summary>
    /// Adds everything still in the field, as finished, and checks the list against the organiser.
    /// A Save does this first, so an address typed and not added is not lost; <c>false</c> when
    /// something cannot be invited, and <see cref="Refusal"/> says why.
    /// </summary>
    internal bool TakeTyped()
    {
        var token = RecipientTokens.CurrentToken(Field);
        IReadOnlyList<string> finished = token.Length == 0
            ? RecipientTokens.Committed(Field)
            : [.. RecipientTokens.Committed(Field), token];
        var taken = Take(finished, string.Empty) && Recheck();
        Changed?.Invoke();
        return taken;
    }

    /// <summary>
    /// Whether nobody on the list is the organiser, who on a create follows the calendar picked,
    /// so an address invited under one calendar can be the organiser of the next. <c>false</c>
    /// sets <see cref="Refusal"/>; a refusal of that kind clears once the clash is gone.
    /// </summary>
    internal bool Recheck()
    {
        var organiser = _organiser();
        if (_invitees.Any(invitee => _checkOf(invitee.Email, [], organiser) is InviteeCheck.Organiser))
        {
            Refusal = new InviteeCheck.Organiser();
            return false;
        }
        if (Refusal is InviteeCheck.Organiser && Field.Trim().Length == 0)
        {
            Refusal = null;
        }
        return true;
    }

    internal void Remove(int index)
    {
        _invitees.RemoveAt(index);
        Recheck();
        Changed?.Invoke();
    }

    internal void SetOptional(int index, bool optional)
    {
        var role = optional ? MeetingInviteeRole.Optional : MeetingInviteeRole.Required;
        _invitees[index] = _invitees[index] with { Role = role };
        Changed?.Invoke();
    }

    // Adds the finished addresses in order. The first one the core refuses stops it, and that one
    // and everything after it stay in the field; a refused last address goes back into the input
    // rather than becoming a finished one, so it is the text the caret is in.
    private bool Take(IReadOnlyList<string> finished, string token)
    {
        for (var index = 0; index < finished.Count; index++)
        {
            if (!Add(finished[index]))
            {
                var rest = finished.Skip(index).ToList();
                Field = token.Length > 0
                    ? RecipientTokens.FieldText(rest, token)
                    : RecipientTokens.FieldText(rest[..^1], rest[^1]);
                return false;
            }
        }
        Field = token;
        return true;
    }

    private bool Add(string typed)
    {
        // The rows nobody edits here are on the meeting too, so typing one is not a new invitee.
        var invited = (_editing?.Attendees ?? [])
            .Where(attendee => !attendee.IsOrganizer && attendee.Role is null)
            .Select(attendee => attendee.Email)
            .Concat(_invitees.Select(invitee => invitee.Email))
            .ToList();
        var check = _checkOf(typed, invited, _organiser());
        if (check is InviteeCheck.Valid valid)
        {
            _invitees.Add(new MeetingInvitee(null, valid.Address, MeetingInviteeRole.Required));
            Refusal = null;
            return true;
        }
        Refusal = check;
        return false;
    }
}
