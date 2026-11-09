// Asks whether to follow a calendar or address book to a server on another site
// (docs/account-autodetect.md rule 14), from Settings → Accounts and from the outage lines above
// the week and the contacts list. What it says is the core's `AccountServerChange`, read again on
// every Settings and Notices signal; Allow is a change on the account's page.
//
// A sheet rather than an `.alert`: the two sites are what the person is judging, so they are set in
// bold, and an alert's message takes neither styling nor four paragraphs. No button is the default,
// as the HIG asks of an alert the person should read: Return allows nothing, Escape and a dismissed
// sheet are "Not now", which needs no call because the use stays paused.

import MailcalBindings
import SwiftUI

/// The question for one account, as the person opened it with "Review".
struct ServerChangeReview: Identifiable {
    let account: String
    /// The account as a person knows it, for the body line.
    let address: String

    var id: String { account }
}

extension MailboxModel {
    /// Opens the question `id` is paused for; `nil` once nothing waits.
    func serverChangeReview(_ id: String) -> ServerChangeReview? {
        guard let question = app?.accountServerChange(accountId: id) else { return nil }
        serverChanges[id] = question
        return ServerChangeReview(account: id, address: signInName(id))
    }

    /// Reads every open question again: an allow starting or ending, or a lookup replacing it. One
    /// the account no longer waits on is dropped, which closes its sheet.
    func refreshServerChanges() {
        guard let app else { return }
        for id in Array(serverChanges.keys) {
            let fresh = app.accountServerChange(accountId: id)
            if fresh != serverChanges[id] { serverChanges[id] = fresh }
        }
    }

    /// Allows the change the question shows; the core moves the account onto it only once it
    /// connects.
    func allowServerChange(_ account: String, shown: ServerChange) {
        changeAccount(.allowServerChange(account: account, shown: shown))
    }
}

extension View {
    /// Presents the server-change question `review` holds, and clears it once answered.
    func serverChangeReview(
        _ model: MailboxModel, _ review: Binding<ServerChangeReview?>
    ) -> some View {
        sheet(item: review) { shown in
            ServerChangeDialog(model: model, review: shown) {
                model.serverChanges[shown.account] = nil
                review.wrappedValue = nil
            }
        }
    }
}

struct ServerChangeDialog: View {
    var model: MailboxModel
    let review: ServerChangeReview
    let close: () -> Void

    var body: some View {
        let question = model.serverChanges[review.account]
        Group {
            if let question {
                content(question)
            }
        }
        // Settled, here or elsewhere: nothing waits any more.
        .onChange(of: question == nil, initial: true) { _, gone in
            if gone { close() }
        }
    }

    private func content(_ question: AccountServerChange) -> some View {
        // Closing would not stop an Allow under way, so "Not now" waits for it too.
        let allowing = question.allow.running
        return VStack(alignment: .leading, spacing: 16) {
            // Scrolls on a phone, where a large text size can outgrow the sheet; a macOS sheet
            // sizes to what it holds.
            #if os(iOS)
                ScrollView { words(question) }
            #else
                words(question)
            #endif
            HStack(spacing: 10) {
                Spacer()
                ControlButton(
                    title: L10n.server_changed_not_now(), role: .cancel, control: .free(!allowing),
                    action: close
                )
                .keyboardShortcut(.cancelAction)
                ControlButton(title: L10n.server_changed_allow(), control: question.allow) {
                    model.allowServerChange(review.account, shown: question.change)
                }
            }
            .buttonStyle(.bordered)
            #if os(iOS)
                .controlSize(.large)
            #endif
        }
        .padding(20)
        .interactiveDismissDisabled(allowing)
        // A macOS sheet has no intrinsic width to derive from a wrapping paragraph, so it must be
        // given one; on iOS the sheet is the screen's width and a fixed one would clip.
        #if os(macOS)
            .frame(width: 440)
        #else
            .presentationDetents([.medium, .large])
        #endif
    }

    private func words(_ question: AccountServerChange) -> some View {
        let change = question.change
        return VStack(alignment: .leading, spacing: 12) {
            Label(ServerChangeLabels.title(change), systemImage: "exclamationmark.triangle")
                .font(.headline)
            Text(ServerChangeLabels.body(change, address: review.address))
            VStack(alignment: .leading, spacing: 4) {
                Text(ServerChangeLabels.before(change))
                Text(ServerChangeLabels.now(change))
            }
            .padding(.leading, 12)
            Text(L10n.server_changed_advice())
            Text(ServerChangeLabels.unsure(change))
            if question.failed {
                Text(L10n.server_changed_failed())
                    .font(.callout)
                    .foregroundStyle(.red)
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
