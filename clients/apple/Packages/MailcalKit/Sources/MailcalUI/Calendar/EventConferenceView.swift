// An event's video meetings in its detail (docs/conferencing.md): Join first, then what a person
// needs to get in another way, the meeting ID, the passcode and the numbers to dial.
//
// Which meetings exist, which link Join opens and which numbers are dialled is the core's
// (`EventDetail.conferences`). Every string here came from whoever wrote the event, so it is drawn
// as `Text`, and the links open through the OS, never in a view of the app's own.

import MailcalBindings
import SwiftUI

/// One meeting, as a section of the detail list.
struct EventConferenceSection: View {
    let conference: EventConference
    @Environment(\.openURL) private var openURL

    var body: some View {
        Section {
            let join = conferenceJoinURL(conference)
            HStack(spacing: 12) {
                Label(conferenceTitle(conference), systemImage: "video")
                Spacer(minLength: 8)
                if let join {
                    Button(L10n.event_join()) { openURL(join) }
                        .buttonStyle(.borderedProminent)
                        .accessibilityIdentifier("event-join")
                }
            }
            if let status = conferenceStatus(conference) {
                Text(status).foregroundStyle(.secondary)
            }
            // The same gate as Join: a link Join will not open is not offered to copy either.
            if let join {
                Button(L10n.action_copy_link(), systemImage: "doc.on.doc") {
                    PlatformPasteboard.copy(join.absoluteString)
                }
            }
            if let id = conference.meetingId {
                labeled(L10n.event_meeting_id(), id)
            }
            if let passcode = conference.passcode {
                labeled(L10n.event_meeting_passcode(), passcode)
            }
            if !conference.dialIn.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L10n.event_meeting_dial_in()).font(.caption).foregroundStyle(.tint)
                    // By position: one number can be listed twice, with a different PIN.
                    ForEach(Array(conference.dialIn.enumerated()), id: \.offset) { _, number in
                        DialInRow(number: number)
                    }
                }
            }
            if let notes = conference.notes, !notes.isEmpty {
                Text(notes).font(.callout).foregroundStyle(.secondary).textSelection(.enabled)
            }
        }
    }

    private func labeled(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption).foregroundStyle(.tint)
            Text(value).textSelection(.enabled)
        }
    }
}

private struct DialInRow: View {
    let number: ConferenceDialIn
    @Environment(\.openURL) private var openURL

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            if let url = dialInURL(number) {
                Button { openURL(url) } label: {
                    Label(number.number, systemImage: "phone")
                }
                .buttonStyle(.borderless)
            } else {
                Text(number.number)
            }
            if let pin = number.pin {
                Text(L10n.event_meeting_pin(pin: pin))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            }
        }
    }
}

/// The meeting's heading: the service's own name, a brand the same in every language, or the
/// catalog's words for a video meeting when nothing names one.
func conferenceTitle(_ conference: EventConference) -> String {
    conference.service ?? L10n.event_video_meeting()
}

/// What to say instead of Join while the server is still making the meeting, or after it could
/// not; `nil` for a meeting that is ready.
func conferenceStatus(_ conference: EventConference) -> String? {
    switch conference.state {
    case .ready: return nil
    case .preparing: return L10n.event_meeting_preparing()
    case .failed: return L10n.event_meeting_failed()
    }
}

/// The link Join opens, once more through the core's own gate for links a person taps, which is
/// the one every other link in the app passes.
func conferenceJoinURL(_ conference: EventConference) -> URL? {
    guard let link = conference.joinUrl, shouldOpenExternalLink(url: link) else { return nil }
    return URL(string: link)
}

/// The `tel:` URI a dial-in row opens. The core builds it from a validated number, and the
/// external-link gate admits web and mail links only, so this checks the scheme itself.
func dialInURL(_ number: ConferenceDialIn) -> URL? {
    guard number.uri.lowercased().hasPrefix("tel:"), let url = URL(string: number.uri) else {
        return nil
    }
    return url
}

/// An event's title on a block, bar or chip, with the camera after it when the event has a video
/// meeting (`hasConference`). The title truncates first, so the camera stays in sight. Hidden from
/// VoiceOver: the element's own label says "video meeting" (`calendarEventLabel`).
struct EventTitleLine: View {
    let title: String
    let hasConference: Bool
    var lineLimit = 1

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 3) {
            Text(title).lineLimit(lineLimit)
            if hasConference {
                Image(systemName: "video")
                    .imageScale(.small)
                    .accessibilityHidden(true)
            }
        }
    }
}
