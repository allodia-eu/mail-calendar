// Who is on an event. Shared by the detail sheet and the editor, because the two must not describe
// the same meeting differently.
//
// Read-only everywhere: changing an attendee list means sending iTIP updates to the people on it,
// which is a separate feature. The editor therefore *shows* the list and says it cannot be changed
// here, rather than offering a control that would silently drop the change.
//
// Every string on these rows is attacker-controlled, it came from whoever sent the invitation. The
// core has already stripped control characters and bidi overrides and bounded the length; SwiftUI's
// Text renders it as text, so there is nothing further to escape.

import MailcalBindings
import SwiftUI

/// The attendee rows, in the order the core put them (organiser first).
struct EventAttendeesSection: View {
    let attendees: [EventAttendee]

    var body: some View {
        ForEach(attendees, id: \.email) { attendee in
            AttendeeRow(attendee: attendee)
        }
    }
}

private struct AttendeeRow: View {
    let attendee: EventAttendee

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            VStack(alignment: .leading, spacing: 1) {
                // An attendee with no display name is shown by address rather than by an invented
                // name, so the second line is dropped instead of repeating the first.
                Text(attendee.name.isEmpty ? attendee.email : attendee.name)
                if let subtitle = attendeeSubtitle(attendee) {
                    Text(subtitle).font(.caption).foregroundStyle(.secondary)
                }
            }
            Spacer(minLength: 8)
            Text(attendeeResponseText(attendee.response))
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}

/// The second line under an attendee: their address (when the first line used their name) and
/// whether they called the meeting. `nil` when there is nothing left to say.
func attendeeSubtitle(_ attendee: EventAttendee) -> String? {
    var parts: [String] = []
    if !attendee.name.isEmpty { parts.append(attendee.email) }
    if attendee.isOrganizer { parts.append(L10n.event_attendee_organizer()) }
    return parts.isEmpty ? nil : parts.joined(separator: " · ")
}

/// How one attendee answered, localised. Third person, this is somebody else's answer, unlike the
/// invitation card's "You accepted".
func attendeeResponseText(_ response: ResponseStatus) -> String {
    switch response {
    case .accepted: return L10n.event_attendee_accepted()
    case .declined: return L10n.event_attendee_declined()
    case .tentative: return L10n.event_attendee_tentative()
    case .delegated: return L10n.event_attendee_delegated()
    case .needsAction: return L10n.event_attendee_needs_action()
    }
}
