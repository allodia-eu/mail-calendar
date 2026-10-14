// Who a meeting invites, as the event editor's controls hold it: a value type, for the reason
// EventEditorState is one, so the MailcalKit tests drive it without a view.
//
// The list holds the rows an organiser may edit, never the organiser. Addresses arrive through the
// composer's own recipient field, so they complete from the same people index as To does, and each
// finished address moves out of the field into a row of its own. Whether one may be added is the
// core's answer (`checkMeetingInvitee`), and what a save sends is worked out from the list by the
// core too (`meetingInviteePatch`), so this type decides neither.

import MailcalBindings

struct InviteeRoster {
    /// The addresses on the meeting that no control here may change (a chair, a role the editor
    /// cannot state): on the meeting all the same, so typing one is not a new invitee.
    let fixed: [String]
    /// Whether this is a stored meeting with nobody but its editable rows invited, which is what
    /// makes its last invitee stay.
    private let storedWithoutFixed: Bool

    private(set) var invitees: [MeetingInvitee]
    /// The field's value, comma-separated as the composer's are.
    private(set) var field = ""
    /// Why the last address could not be added, or `nil` when nothing was refused.
    private(set) var refusal: InviteeCheck?

    init(editing: EditTarget?) {
        let stored = editing?.attendees ?? []
        fixed = stored.filter { !$0.isOrganizer && $0.role == nil }.map(\.email)
        storedWithoutFixed = !stored.isEmpty && fixed.isEmpty
        invitees = editing?.canEditInvitees == true
            ? stored.compactMap { attendee in
                guard !attendee.isOrganizer, let role = attendee.role else { return nil }
                return MeetingInvitee(
                    name: attendee.name.isEmpty ? nil : attendee.name, email: attendee.email, role: role
                )
            }
            : []
    }

    /// Whether the one invitee left on an existing meeting must stay: a meeting is called off by
    /// deleting it, and a roster change that leaves nobody invited is refused. A row nobody edits
    /// here, such as a chair, still counts as invited.
    var keepsLastInvitee: Bool { invitees.count == 1 && storedWithoutFixed }

    /// The field changed: every finished address in it moves into the list. One the core refuses
    /// stays in the field, with `refusal` saying why.
    mutating func fieldChanged(_ value: String, organiser: String) {
        var refused: String?
        for address in committedRecipients(value) where !add(address, organiser: organiser) {
            refused = refused ?? address
        }
        field = refused ?? currentRecipientToken(value)
    }

    /// Adds what is still being typed. Save does this first, so an address typed and not added is
    /// not lost; `false` when it cannot be invited, and `refusal` says why.
    mutating func takeTyped(organiser: String) -> Bool {
        let typed = currentRecipientToken(field)
        if typed.isEmpty { return true }
        let added = add(typed, organiser: organiser)
        if added { field = "" }
        return added
    }

    mutating func remove(at index: Int) {
        guard invitees.indices.contains(index) else { return }
        invitees.remove(at: index)
    }

    mutating func setOptional(_ optional: Bool, at index: Int) {
        guard invitees.indices.contains(index) else { return }
        invitees[index].role = optional ? .optional : .required
    }

    private mutating func add(_ typed: String, organiser: String) -> Bool {
        let check = checkMeetingInvitee(
            typed: typed, invited: fixed + invitees.map(\.email), organiser: organiser
        )
        guard case .valid(let address) = check else {
            refusal = check
            return false
        }
        invitees.append(MeetingInvitee(name: nil, email: address, role: .required))
        refusal = nil
        return true
    }
}
