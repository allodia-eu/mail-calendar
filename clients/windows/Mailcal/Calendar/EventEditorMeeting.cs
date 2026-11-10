// The editor's Video meeting row (docs/conferencing.md, rules 9 to 12): which meetings it offers,
// what it opens showing, and what a save sends. The core decides what a calendar offers
// (CalendarRow.MeetingChoices) and which meeting the editor opens on (EventDetail.EditorMeeting);
// this only holds the person's pick and compares it with what the editor opened with. Free of
// WinUI and of the catalog, so Mailcal.Tests drives it as it drives the rest of the editor.
using System;
using System.Collections.Generic;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Calendar;

/// <summary>What the Video meeting row holds.</summary>
internal abstract record MeetingSelection
{
    private MeetingSelection()
    {
    }

    /// <summary>No meeting.</summary>
    internal sealed record None : MeetingSelection;

    /// <summary>One of the calendar's <c>MeetingChoices</c>, by its id.</summary>
    internal sealed record Choice(string Id) : MeetingSelection;

    /// <summary>The meeting the event already has that no choice makes (a Zoom link from an
    /// invitation), kept as it is.</summary>
    internal sealed record Current : MeetingSelection;

    /// <summary>What an editor opens with for <paramref name="meeting"/>, the event's current
    /// meeting.</summary>
    internal static MeetingSelection Of(EditorMeeting? meeting) => meeting switch
    {
        null => new None(),
        { Choice: { } id } => new Choice(id),
        _ => new Current(),
    };
}

/// <summary>What a save does to the meeting, before the catalog's words are put with it.</summary>
internal abstract record MeetingPick
{
    private MeetingPick()
    {
    }

    /// <summary>Take the event's meeting off.</summary>
    internal sealed record Remove : MeetingPick;

    /// <summary>Put the calendar's choice <paramref name="Id"/> on the event.</summary>
    internal sealed record Set(string Id) : MeetingPick;

    /// <summary>The core's change for this pick, with the words the notes carry for a meeting
    /// made elsewhere.</summary>
    internal MeetingChange Change(MeetingWords words) => this switch
    {
        Set set => new MeetingChange.Set(set.Id, words),
        _ => new MeetingChange.Remove(),
    };
}

/// <summary>
/// One entry the row offers. <paramref name="Name"/> is the service's brand name, or null for the
/// dialog's own words: "None" for <see cref="MeetingSelection.None"/>, "Video meeting" for a
/// meeting nothing names. <paramref name="Through"/> is the address of the account that makes it,
/// for a meeting made through an account (docs/conferencing.md rule 14).
/// </summary>
internal sealed record MeetingRowOption(MeetingSelection Selection, string? Name, string? Through = null)
{
    /// <summary>The words this option is shown by: the service's brand name, with the account
    /// that makes it through <paramref name="through"/> (<c>event_meeting_through</c>), else the
    /// catalog's.</summary>
    internal string Label(string none, string generic, Func<string, string, string> through) =>
        Selection is MeetingSelection.None ? none
            : Name is { } name && Through is { } account ? through(name, account)
            : Name ?? generic;
}

internal sealed partial class EventEditorState
{
    private MeetingSelection _meeting = new MeetingSelection.None();

    /// <summary>The person's pick in the Video meeting row.</summary>
    internal MeetingSelection Meeting
    {
        get => _meeting;
        set
        {
            _meeting = value;
            MeetingChosen = true;
        }
    }

    /// <summary>Whether the person picked in the row. Until they do, a new event's row follows its
    /// calendar's default (docs/conferencing.md, rule 10).</summary>
    internal bool MeetingChosen { get; private set; }

    /// <summary>What this editor opened with.</summary>
    internal MeetingSelection OpenedMeeting => MeetingSelection.Of(Editing?.Meeting);

    private IReadOnlyList<MeetingOption> MeetingChoices => Calendar?.MeetingChoices ?? [];

    /// <summary>
    /// The row's entries: None first, the event's own meeting where the calendar does not offer it
    /// (one no choice makes, or a Jitsi meeting after the server was cleared), then the calendar's
    /// choices by the core's names.
    /// </summary>
    internal IReadOnlyList<MeetingRowOption> MeetingOptions
    {
        get
        {
            var options = new List<MeetingRowOption> { new(new MeetingSelection.None(), null) };
            if (Editing?.Meeting is { } meeting && MeetingChoices.All(choice => choice.Id != meeting.Choice))
            {
                options.Add(new MeetingRowOption(OpenedMeeting, meeting.Name));
            }
            options.AddRange(MeetingChoices.Select(
                choice => new MeetingRowOption(new MeetingSelection.Choice(choice.Id), choice.Name, choice.Through)));
            return options;
        }
    }

    /// <summary>Whether the row is drawn: the calendar offers a meeting, or the event has one to
    /// take off.</summary>
    internal bool ShowsMeetingRow => MeetingChoices.Count > 0 || Editing?.Meeting is not null;

    /// <summary>
    /// What the row holds before the calendar is asked whether it offers it: the person's pick
    /// once they made one, and until then, on a new event with an invitee, the calendar's default
    /// (rule 10). An event being edited keeps the meeting it has.
    /// </summary>
    private MeetingSelection HeldMeeting
    {
        get
        {
            if (IsEditing || MeetingChosen)
            {
                return Meeting;
            }
            return CanInvite && Roster.Invitees.Count > 0 && Calendar?.DefaultMeeting is { } id
                ? new MeetingSelection.Choice(id)
                : new MeetingSelection.None();
        }
    }

    /// <summary>
    /// What the row shows and a save reads: a pick the chosen calendar does not offer goes back to
    /// None when the calendar changes (rule 9), unless it is what the editor opened with.
    /// </summary>
    internal MeetingSelection EffectiveMeeting
    {
        get
        {
            var held = HeldMeeting;
            if (held is not MeetingSelection.Choice pick || pick == OpenedMeeting)
            {
                return held;
            }
            return MeetingChoices.Any(choice => choice.Id == pick.Id) ? pick : new MeetingSelection.None();
        }
    }

    /// <summary>
    /// Whether the line beneath the row says the link is added at the save (rule 10): the row
    /// holds a choice the event does not have yet. A meeting is made when the event is saved, never
    /// when it is chosen.
    /// </summary>
    internal bool AnnouncesMeeting =>
        EffectiveMeeting is MeetingSelection.Choice pick && pick != OpenedMeeting;

    /// <summary>
    /// What a save does to the meeting, or null to leave it alone: None takes the event's meeting
    /// off, a choice replaces it, and leaving the row as it opened changes nothing.
    /// </summary>
    internal MeetingPick? MeetingPick
    {
        get
        {
            var pick = EffectiveMeeting;
            if (pick == OpenedMeeting)
            {
                return null;
            }
            return pick switch
            {
                MeetingSelection.None => IsEditing ? new MeetingPick.Remove() : null,
                MeetingSelection.Choice choice => new MeetingPick.Set(choice.Id),
                _ => null,
            };
        }
    }
}
