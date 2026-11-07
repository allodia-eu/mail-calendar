package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.EditorMeeting
import uniffi.mailcal_bindings.MeetingChange
import uniffi.mailcal_bindings.MeetingWords

// The editor's Video meeting row (docs/conferencing.md, rules 9 to 14): which meetings it offers,
// what it opens showing, and what a save sends. The core decides what a calendar offers
// (`CalendarRow.meetingChoices`) and which meeting the editor opens on (`EventDetail.editorMeeting`);
// this only holds the person's pick and compares it with what the editor opened with. Kept apart
// from EventEditor.kt, and free of a Context, so the JVM suite drives it as it drives the rest.

/** What the Video meeting row holds. */
internal sealed interface MeetingSelection {
    /** No meeting. */
    data object None : MeetingSelection

    /** One of the calendar's `meetingChoices`, by its id. */
    data class Choice(val id: String) : MeetingSelection

    /** The meeting the event already has that no choice makes, kept as it is. */
    data object Current : MeetingSelection
}

/** What a save does to the meeting, before the catalog's words are put with it. */
internal sealed interface MeetingPick {
    /** Take the event's meeting off. */
    data object Remove : MeetingPick

    /** Put the calendar's choice [id] on the event. */
    data class Set(val id: String) : MeetingPick
}

/**
 * One entry the row offers. [name] is the service's brand name, or `null` for the screen's own
 * words: "None" for [MeetingSelection.None], "Video meeting" for a meeting nothing names. [through]
 * is the address of the account a choice is made through, for one made through an account.
 */
internal data class MeetingRowOption(
    val selection: MeetingSelection,
    val name: String?,
    val through: String? = null,
)

/** What an editor opens with for [meeting], the event's current meeting. */
internal fun meetingSelectionOf(meeting: EditorMeeting?): MeetingSelection {
    if (meeting == null) return MeetingSelection.None
    return meeting.choice?.let { MeetingSelection.Choice(it) } ?: MeetingSelection.Current
}

/** What this editor opened with. */
internal val EventEditorState.openedMeeting: MeetingSelection
    get() = meetingSelectionOf(editing?.meeting)

/**
 * The row's entries: None first, the event's own meeting where the calendar does not offer it
 * (one no choice makes, or a Jitsi meeting after the server was cleared), then the calendar's
 * choices by the core's names, one made through an account with its address.
 */
internal val EventEditorState.meetingOptions: List<MeetingRowOption>
    get() {
        val choices = calendar?.meetingChoices.orEmpty()
        val current = editing?.meeting
            ?.takeIf { meeting -> choices.none { it.id == meeting.choice } }
            ?.let { MeetingRowOption(openedMeeting, it.name) }
        return listOf(MeetingRowOption(MeetingSelection.None, null)) +
            listOfNotNull(current) +
            choices.map { MeetingRowOption(MeetingSelection.Choice(it.id), it.name, it.through) }
    }

/** Whether the row is drawn: the calendar offers a meeting, or the event has one to take off. */
internal val EventEditorState.showsMeetingRow: Boolean
    get() = calendar?.meetingChoices.orEmpty().isNotEmpty() || editing?.meeting != null

/**
 * Whether the row says the link is added at the save: it holds a choice the event does not have
 * yet, which is made only when the event is saved (rule 10).
 */
internal val EventEditorState.announcesMeeting: Boolean
    get() {
        val effective = effectiveMeeting
        return effective is MeetingSelection.Choice && effective != openedMeeting
    }

/**
 * The pick a save reads: one the chosen calendar does not offer goes back to None when the
 * calendar changes (rule 9), unless it is what the editor opened with.
 */
internal val EventEditorState.effectiveMeeting: MeetingSelection
    get() {
        val pick = meeting
        if (pick !is MeetingSelection.Choice || pick == openedMeeting) return pick
        val offered = calendar?.meetingChoices.orEmpty().any { it.id == pick.id }
        return if (offered) pick else MeetingSelection.None
    }

/**
 * What a save does to the meeting, or `null` to leave it alone: None takes the event's meeting
 * off, a choice replaces it, and leaving the row as it opened changes nothing.
 */
internal val EventEditorState.meetingPick: MeetingPick?
    get() {
        val pick = effectiveMeeting
        if (pick == openedMeeting) return null
        return when (pick) {
            MeetingSelection.None -> if (isEditing) MeetingPick.Remove else null
            MeetingSelection.Current -> null
            is MeetingSelection.Choice -> MeetingPick.Set(pick.id)
        }
    }

/** The core's change for this pick, with the words the notes carry for a meeting made elsewhere. */
internal fun MeetingPick.change(words: MeetingWords): MeetingChange = when (this) {
    MeetingPick.Remove -> MeetingChange.Remove
    is MeetingPick.Set -> MeetingChange.Set(id, words)
}

/** The catalog's words for the block, in the app's language: the core has no locale. */
internal fun meetingWords(ctx: Context): MeetingWords = MeetingWords(
    meeting = L10n.event_video_meeting(ctx),
    join = L10n.event_join(ctx),
    meetingId = L10n.event_meeting_id(ctx),
    passcode = L10n.event_meeting_passcode(ctx),
    dialIn = L10n.event_meeting_dial_in(ctx),
    dialInCode = L10n.event_meeting_conference_id(ctx),
)
