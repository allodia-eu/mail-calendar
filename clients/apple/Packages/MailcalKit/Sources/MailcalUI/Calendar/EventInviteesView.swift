// The meeting's invitee list inside the event editor: who it invites, and whether each one is
// required or optional, as the organiser edits it.
//
// Offered only where the core says a meeting may be organised (docs/calendar.md, "Organising a
// meeting"): on a create, a calendar whose server sends the invitations; on an edit, a meeting this
// account organises. Somebody else's meeting shows its list read-only, with the sentence saying so,
// and a create on a calendar that cannot invite says why nobody can be added. The state is
// InviteeRoster's; this file only draws it.

import MailcalBindings
import SwiftUI

struct EventInviteesSection: View {
    @Binding var editor: EventEditorState
    let suggestionsFor: ((String) async -> [RecipientMatch])?

    var body: some View {
        let attendees = editor.editing?.attendees ?? []
        if !editor.canInvite {
            if editor.editing == nil {
                Section { note(L10n.event_invitees_unavailable()) }
            } else if !attendees.isEmpty {
                // Shown so an edit is not made blind to who is coming, and stated to be read-only
                // rather than offered as a field that would quietly drop the change.
                Section {
                    EventAttendeesSection(attendees: attendees)
                    note(L10n.event_attendees_read_only())
                } header: {
                    Text(L10n.event_attendees())
                }
            }
        } else {
            Section {
                // The rows no control here may change: the organiser, and a role the editor
                // cannot state.
                EventAttendeesSection(attendees: attendees.filter { $0.isOrganizer || $0.role == nil })
                ForEach(Array(editor.roster.invitees.enumerated()), id: \.element.email) { index, invitee in
                    InviteeRow(
                        invitee: invitee,
                        answer: attendees
                            .first { $0.email.caseInsensitiveCompare(invitee.email) == .orderedSame }
                            .map { attendeeResponseText($0.response) },
                        removable: !editor.roster.keepsLastInvitee,
                        optional: Binding(
                            get: { invitee.role == .optional },
                            set: { editor.roster.setOptional($0, at: index) }
                        ),
                        remove: { editor.roster.remove(at: index) }
                    )
                }
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    RecipientField(
                        label: L10n.event_invitee_field(),
                        text: Binding(
                            get: { editor.roster.field },
                            set: { editor.inviteeFieldChanged($0) }
                        ),
                        suggestionsFor: suggestionsFor
                    )
                    // As the editor's other fields, and for the same reason: see EventEditorView.
                    .labelsHidden()
                    .textFieldStyle(.roundedBorder)
                    .onSubmit { _ = editor.takeTypedInvitee() }
                    Button(L10n.event_invitee_add()) { _ = editor.takeTypedInvitee() }
                        .buttonStyle(.bordered)
                }
                if let refusal = editor.roster.refusal, let text = inviteeRefusal(refusal) {
                    Text(text)
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
                note(L10n.event_invitees_send_note())
            } header: {
                Text(L10n.event_attendees())
            }
        }
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .font(.footnote)
            .foregroundStyle(.secondary)
    }
}

private struct InviteeRow: View {
    let invitee: MeetingInvitee
    let answer: String?
    let removable: Bool
    @Binding var optional: Bool
    let remove: () -> Void

    var body: some View {
        let shown = invitee.name.flatMap { $0.isEmpty ? nil : $0 } ?? invitee.email
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 1) {
                Text(shown).lineLimit(1).truncationMode(.middle)
                if shown != invitee.email {
                    Text(invitee.email).font(.caption).foregroundStyle(.secondary)
                }
                if let answer {
                    Text(answer).font(.caption).foregroundStyle(.secondary)
                }
                // iOS has no checkbox, and a switch beside the name leaves a phone too little
                // room for the address, so there it takes a line of its own.
                #if !os(macOS)
                    optionalToggle.font(.subheadline).padding(.top, 4)
                #endif
            }
            Spacer(minLength: 8)
            #if os(macOS)
                optionalToggle.toggleStyle(.checkbox)
            #endif
            Button(action: remove) {
                Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
            }
            .buttonStyle(.borderless)
            .disabled(!removable)
            // Named for the person, so the fourth of four identical buttons is still
            // distinguishable to a screen reader; and, when the core would refuse it, why not.
            .accessibilityLabel(L10n.event_invitee_remove(name: shown))
            .accessibilityHint(removable ? "" : L10n.event_invitee_last())
            .help(removable ? L10n.event_invitee_remove(name: shown) : L10n.event_invitee_last())
        }
    }

    /// One toggle carrying its own label, so a screen reader announces the control and what it
    /// means together.
    private var optionalToggle: some View {
        Toggle(L10n.event_attendee_optional(), isOn: $optional)
    }
}

/// What the field says when an address cannot be added; `nil` for one that can.
func inviteeRefusal(_ check: InviteeCheck) -> String? {
    switch check {
    case .valid: return nil
    case .notAnAddress: return L10n.event_invitee_not_an_address()
    case .alreadyInvited: return L10n.event_invitee_already_invited()
    case .organiser: return L10n.event_invitee_organiser()
    }
}
