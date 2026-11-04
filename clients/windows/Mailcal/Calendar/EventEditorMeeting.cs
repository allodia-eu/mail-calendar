// The editor's Video meeting row (docs/conferencing.md, rules 9 to 12): which meetings it offers,
// what it opens showing, and what a save sends. The core decides what a calendar offers
// (CalendarRow.MeetingChoices) and which meeting the editor opens on (EventDetail.EditorMeeting);
// this only holds the person's pick and compares it with what the editor opened with. Free of
// WinUI and of the catalog, so Mailcal.Tests drives it as it drives the rest of the editor.
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
/// meeting nothing names.
/// </summary>
internal sealed record MeetingRowOption(MeetingSelection Selection, string? Name)
{
    /// <summary>The words this option is shown by: the service's brand name, else the
    /// catalog's.</summary>
    internal string Label(string none, string generic) =>
        Selection is MeetingSelection.None ? none : Name ?? generic;
}

internal sealed partial class EventEditorState
{
    /// <summary>What the Video meeting row holds.</summary>
    internal MeetingSelection Meeting { get; set; } = new MeetingSelection.None();

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
                choice => new MeetingRowOption(new MeetingSelection.Choice(choice.Id), choice.Name)));
            return options;
        }
    }

    /// <summary>Whether the row is drawn: the calendar offers a meeting, or the event has one to
    /// take off.</summary>
    internal bool ShowsMeetingRow => MeetingChoices.Count > 0 || Editing?.Meeting is not null;

    /// <summary>
    /// The pick a save reads: one the chosen calendar does not offer goes back to None when the
    /// calendar changes (rule 9), unless it is what the editor opened with.
    /// </summary>
    internal MeetingSelection EffectiveMeeting
    {
        get
        {
            if (Meeting is not MeetingSelection.Choice pick || pick == OpenedMeeting)
            {
                return Meeting;
            }
            return MeetingChoices.Any(choice => choice.Id == pick.Id) ? pick : new MeetingSelection.None();
        }
    }

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
