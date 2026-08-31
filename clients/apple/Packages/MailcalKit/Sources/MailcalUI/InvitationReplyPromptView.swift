// The "the organiser wasn't told" modal: what a calendar server's own report turns into when it
// says it could not deliver the reply it promised to send (RFC 6638 §3.2.9, docs/invitations.md).
//
// A sheet rather than an `.alert`, and that is forced rather than chosen: SwiftUI's alert takes
// only buttons, and the remembered per-account choice has to be a control the user can see the
// state of *before* they commit to it. Encoding it in the buttons instead would mean four of them
// ("send" × "always"), which is a menu, not a question.

import SwiftUI

#if canImport(MailcalBindings)
    import MailcalBindings
#endif

/// Asks whether to email the invitation reply ourselves, after the calendar server said it could
/// not.
///
/// # What this must not be mistaken for
///
/// The RSVP itself **worked**, the answer is stored, and the meeting on screen is correct. What
/// failed is the message to the organiser. So the first line says the answer is saved, and only
/// then what did not happen; a modal that opened with "couldn't send" would invite the user to
/// answer again, which stores the same `PARTSTAT` and fails the same way.
///
/// # Why the recipient is named
///
/// Pressing the button sends mail from the user's account to someone they did not choose in this
/// moment. Consent to that is not informed unless the address is on screen, hence
/// `prompt.organizer` in the sentence rather than the word "the organiser".
///
/// The RFC 6638 status code is deliberately **not** shown. It is carried on the prompt for the
/// diagnostics log; `5.2` in a modal explains nothing to the person reading it.
struct InvitationReplyPromptView: View {
    let prompt: ReplyPrompt
    /// `(send, remember)`, the two independent halves of the answer, exactly as the core's
    /// `AnswerReplyPrompt` takes them. `remember` applies to whichever button was pressed, so a
    /// ticked box plus "Don't send" is a standing *no*, not a standing yes.
    let answer: (Bool, Bool) -> Void

    /// Off by default: a standing choice for every future meeting on this account is a bigger
    /// decision than the one being asked, and it is not the one the user came here to make.
    @State private var remember = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Label(L10n.invitation_reply_undelivered_title(), systemImage: "exclamationmark.triangle")
                .font(.headline)
            // Both values are attacker-controlled, the meeting's title and the organiser's
            // address both come from mail somebody else wrote, so they are interpolated into a
            // `Text`, which renders them as text and never as markup (docs/rendering-security.md).
            Text(
                L10n.invitation_reply_undelivered_body(
                    summary: prompt.summary,
                    organizer: prompt.organizer
                )
            )
            .fixedSize(horizontal: false, vertical: true)
            Toggle(L10n.invitation_reply_undelivered_remember(), isOn: $remember)
            HStack(spacing: 10) {
                Spacer()
                Button(L10n.invitation_reply_undelivered_dismiss()) { answer(false, remember) }
                    .keyboardShortcut(.cancelAction)
                Button(L10n.invitation_reply_undelivered_send()) { answer(true, remember) }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        // A macOS sheet has no intrinsic width to derive from a wrapping paragraph, so it must be
        // given one; on iOS the sheet is the screen's width and a fixed one would clip.
        #if os(macOS)
            .frame(width: 420)
        #else
            .presentationDetents([.medium])
        #endif
    }
}
