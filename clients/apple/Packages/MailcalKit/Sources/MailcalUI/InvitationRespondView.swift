import SwiftUI

#if canImport(MailcalBindings)
    import MailcalBindings
#endif

/// The Accept / Maybe / Decline row, and the two controls that ride beside it on the transports
/// that have them.
///
/// Split out of `InvitationCardView` because it is the only part of the card that *writes*, and
/// because everything in it is conditional on what the account can actually do, the card itself
/// stays a straight render of what the core computed.
///
/// # Three gates, none of them a disabled button
///
/// - **`canRespond`**, the account's calendar cannot RSVP at all. The buttons are then *absent*
///   and a sentence says why. A greyed-out Accept invites the user to try, wonder, and try again;
///   "this account can't send a response" ends it.
/// - **`canComment`**, the transport has nowhere to put a note (CalDAV, JMAP). The field is
///   absent, because the core **refuses** a note it cannot carry rather than dropping it: an
///   offered field would not merely lose the text, it would lose the whole answer.
/// - **`canChooseNotify`**, the server sends the reply the moment the status changes and no
///   client can stop it. The toggle is absent for the same reason: one that emails the organizer
///   anyway is worse than none.
///
/// On both harness accounts, and on any CalDAV or JMAP account, this is three buttons and nothing
/// else. That is the truth of the transport, not a missing feature.
struct InvitationRespondView: View {
    let card: InvitationCard
    let account: String
    let messageKey: String
    let status: CalendarWriteStatus
    let respond: (InvitationResponse, String?, Bool, String) -> Void

    /// The note, kept locally: it exists only until the answer goes out, and re-rendering the card
    /// after the write must not resurrect it.
    @State private var comment = ""
    /// Mirrors the RFC 5546 default: an invitation asks for a reply, so answering sends one. The
    /// user has to say otherwise.
    @State private var notifyOrganizer = true

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if card.canRespond {
                if card.canComment {
                    TextField(L10n.invitation_message_to_organizer(), text: $comment, axis: .vertical)
                        .textFieldStyle(.roundedBorder)
                        .lineLimit(1...3)
                        .font(.caption)
                }
                if card.canChooseNotify {
                    Toggle(L10n.invitation_notify_organizer(), isOn: $notifyOrganizer)
                        .toggleStyle(.switch)
                        .font(.caption)
                }
                buttons
                progress
            } else {
                Text(L10n.invitation_cannot_respond())
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var buttons: some View {
        HStack(spacing: 8) {
            answerButton(.accept, L10n.invitation_accept(), L10n.a11y_invitation_accept(), .borderedProminent)
            answerButton(.tentative, L10n.invitation_tentative(), L10n.a11y_invitation_tentative(), .bordered)
            answerButton(.decline, L10n.invitation_decline(), L10n.a11y_invitation_decline(), .bordered)
        }
    }

    /// One answer. The label is the visible word ("Accept"); the accessibility label says what it
    /// acts on, because three bare verbs read out of context tell a screen-reader user nothing
    /// about which invitation they belong to.
    @ViewBuilder
    private func answerButton(
        _ response: InvitationResponse,
        _ title: String,
        _ spoken: String,
        _ style: some PrimitiveButtonStyle
    ) -> some View {
        Button(title) {
            respond(
                response,
                card.canComment ? comment : nil,
                card.canChooseNotify ? notifyOrganizer : true,
                // The subject for the reply the core may have to email itself, composed here
                // because the summary and the chosen answer are both in hand, and because the
                // core has no locale to compose it with.
                invitationReplySubject(response, card.summary)
            )
        }
        .buttonStyle(style)
        .controlSize(.small)
        .accessibilityLabel(spoken)
        .disabled(status == .saving)
    }

    /// What happened to the answer. `saving` and `failed` are the two the user must see: a reply
    /// the organiser never received, reported as sent, is the failure this whole feature exists to
    /// prevent, so a failure says so in words rather than leaving the old answer on screen.
    @ViewBuilder
    private var progress: some View {
        switch status {
        case .saving:
            Label(L10n.invitation_sending(), systemImage: "arrow.triangle.2.circlepath")
                .font(.caption2)
                .foregroundStyle(.secondary)
        case .failed:
            Label(L10n.invitation_failed(), systemImage: "exclamationmark.triangle")
                .font(.caption2)
                .foregroundStyle(.orange)
        case .saved, .idle:
            EmptyView()
        }
    }
}
