// The editor's Video meeting row (docs/conferencing.md, rules 9 to 14): which meetings it offers,
// what it opens showing, and what a save sends. The core decides what a calendar offers
// (`CalendarRow.meetingChoices`) and which meeting the editor opens on (`EventDetail.editorMeeting`);
// this only holds the person's pick and compares it with what the editor opened with.

import MailcalBindings

/// What the Video meeting row holds.
enum MeetingSelection: Hashable {
    /// No meeting.
    case none
    /// One of the calendar's `meetingChoices`, by its id.
    case choice(String)
    /// The meeting the event already has that no choice makes (a Zoom link from an invitation),
    /// kept as it is.
    case current
}

/// One entry the row offers.
struct MeetingRowOption: Hashable {
    let selection: MeetingSelection
    let label: String
}

extension EventEditorState {
    /// What an editor opens with for `meeting`, the event's current meeting.
    static func meetingSelection(_ meeting: EditorMeeting?) -> MeetingSelection {
        guard let meeting else { return .none }
        return meeting.choice.map { .choice($0) } ?? .current
    }

    /// What this editor opened with.
    var openedMeeting: MeetingSelection { Self.meetingSelection(editing?.meeting) }

    /// The row's entries: None first, the event's own meeting when no choice makes it, then the
    /// calendar's choices by the core's names, one made through an account with its address.
    var meetingOptions: [MeetingRowOption] {
        var options = [MeetingRowOption(selection: .none, label: L10n.event_meeting_none())]
        let choices = calendar?.meetingChoices ?? []
        // The event's meeting is its current value even where the calendar offers it no longer
        // (a Jitsi meeting after the Jitsi server was cleared): it can be kept or taken off.
        if let meeting = editing?.meeting,
            !choices.contains(where: { meeting.choice == $0.id })
        {
            options.append(
                MeetingRowOption(selection: openedMeeting, label: meeting.name ?? L10n.event_video_meeting())
            )
        }
        options += choices.map { MeetingRowOption(selection: .choice($0.id), label: Self.label($0)) }
        return options
    }

    /// A choice as the row names it: the service, and the account that makes it for one made
    /// through an account, so two work accounts are two entries a person can tell apart.
    static func label(_ option: MeetingOption) -> String {
        option.through.map { L10n.event_meeting_through(service: option.name, account: $0) } ?? option.name
    }

    /// Whether the row is drawn: the calendar offers a meeting, or the event has one to take off.
    var showsMeetingRow: Bool {
        !(calendar?.meetingChoices.isEmpty ?? true) || editing?.meeting != nil
    }

    /// The pick a save reads: one the chosen calendar does not offer goes back to None when the
    /// calendar changes (rule 9), unless it is what the editor opened with.
    var effectiveMeeting: MeetingSelection {
        guard case .choice(let id) = meeting, meeting != openedMeeting else { return meeting }
        let offered = calendar?.meetingChoices.contains { $0.id == id } ?? false
        return offered ? meeting : .none
    }

    /// What a save sends for the meeting, or `nil` to leave it alone: None takes the event's
    /// meeting off, a choice replaces it, and leaving the row as it opened changes nothing.
    func meetingChange(words: MeetingWords = .catalog) -> MeetingChange? {
        let pick = effectiveMeeting
        guard pick != openedMeeting else { return nil }
        switch pick {
        case .none: return isEditing ? .remove : nil
        case .current: return nil
        case .choice(let id): return .set(choice: id, words: words)
        }
    }
}

extension MeetingWords {
    /// The catalog's words for the block a meeting made elsewhere leaves in the notes, in the app's
    /// language: the core has no locale.
    static var catalog: MeetingWords {
        MeetingWords(
            meeting: L10n.event_video_meeting(),
            join: L10n.event_join(),
            meetingId: L10n.event_meeting_id(),
            passcode: L10n.event_meeting_passcode(),
            dialIn: L10n.event_meeting_dial_in(),
            dialInCode: L10n.event_meeting_conference_id()
        )
    }
}
