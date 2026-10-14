// An invitee's answer in an event's detail: what they answered, and Accept, Maybe and Decline to
// change it.
//
// Offered only where the core sets `EventDetail.myAnswer`: the account is invited, does not
// organise the meeting, and has a way to tell the organiser. The answer is for the whole meeting,
// and goes the way the invitation card's does (docs/calendar.md, "Answering from the calendar").
// The words are the card's, so the two places a meeting is answered read the same.
//
// Accept and Maybe go at once and the detail stays open; Decline asks first, because a declined
// meeting leaves the calendar and the detail with it.

import MailcalBindings
import SwiftUI

/// The answer row's decisions, without a view.
struct EventAnswer: Equatable {
    /// What this account has answered so far.
    let current: ResponseStatus
    /// Whether a calendar write is on its way: an answer is, and a second would race it.
    let saving: Bool

    init(current: ResponseStatus, saving: Bool) {
        self.current = current
        self.saving = saving
    }

    /// The answer an event's detail offers, or `nil` where the core offers none: this account
    /// organises the meeting, it is an appointment, or nothing would reach the organiser.
    init?(detail: EventDetail, saving: Bool) {
        guard let current = detail.myAnswer else { return nil }
        self.init(current: current, saving: saving)
    }

    /// What pressing an answer does.
    enum Step: Equatable {
        /// Send this answer now.
        case send(InvitationResponse)
        /// Ask before declining; nothing is sent until the user confirms.
        case confirmDecline
        /// Nothing: the button cannot be pressed.
        case none
    }

    /// Whether `response` is the answer already given; that button is shown chosen and inert,
    /// since sending it again changes nothing.
    func isCurrent(_ response: InvitationResponse) -> Bool {
        switch response {
        case .accept: current == .accepted
        case .tentative: current == .tentative
        case .decline: current == .declined
        }
    }

    func canPress(_ response: InvitationResponse) -> Bool {
        !saving && !isCurrent(response)
    }

    func press(_ response: InvitationResponse) -> Step {
        guard canPress(response) else { return .none }
        return response == .decline ? .confirmDecline : .send(response)
    }
}

/// The answer, under a line saying what was answered so far. `onAnswer` gets the reply subject.
struct EventAnswerSection: View {
    let answer: EventAnswer
    let title: String
    let onAnswer: (InvitationResponse, String) -> Void

    @State private var confirmingDecline = false

    var body: some View {
        Section {
            Text(invitationResponseLine(answer.current))
                .font(.callout)
                .foregroundStyle(.tint)
            HStack(spacing: 8) {
                button(.accept, L10n.invitation_accept(), L10n.a11y_invitation_accept())
                button(.tentative, L10n.invitation_tentative(), L10n.a11y_invitation_tentative())
                button(.decline, L10n.invitation_decline(), L10n.a11y_invitation_decline())
            }
        }
        // An `alert`, for the reason the detail's delete confirmation is one: iPadOS draws a
        // confirmation dialog as a popover, which drops its `.cancel` button.
        .alert(L10n.event_decline_confirm_title(), isPresented: $confirmingDecline) {
            Button(L10n.invitation_decline(), role: .destructive) { send(.decline) }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: {
            Text(L10n.event_decline_confirm_body())
        }
    }

    /// One answer. The answer already given is drawn prominent, as chosen; the accessibility label
    /// says what it acts on, as on the invitation card.
    @ViewBuilder
    private func button(_ response: InvitationResponse, _ label: String, _ spoken: String) -> some View {
        let press = {
            switch answer.press(response) {
            case .send(let chosen): send(chosen)
            case .confirmDecline: confirmingDecline = true
            case .none: break
            }
        }
        Group {
            if answer.isCurrent(response) {
                Button(label, action: press).buttonStyle(.borderedProminent)
            } else {
                Button(label, action: press).buttonStyle(.bordered)
            }
        }
        .disabled(!answer.canPress(response))
        .accessibilityLabel(spoken)
    }

    private func send(_ response: InvitationResponse) {
        onAnswer(response, invitationReplySubject(response, title))
    }
}
