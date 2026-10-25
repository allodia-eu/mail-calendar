// Asks whether to follow a calendar or address book to a server on another site
// (docs/account-autodetect.md rule 14), from Settings → Accounts and from the outage lines above
// the week and the contacts list.
//
// A sheet rather than an `.alert`: the two sites are what the person is judging, so they are set in
// bold, and an alert's message takes neither styling nor four paragraphs. No button is the default,
// as the HIG asks of an alert the person should read: Return allows nothing, Escape and a dismissed
// sheet are "Not now", which needs no call because the use stays paused.

import MailcalBindings
import SwiftUI

/// The question for one account, as it stood when the person chose "Review".
struct ServerChangeReview: Identifiable {
    let account: String
    /// The account as a person knows it, for the body line.
    let address: String
    let change: ServerChange

    var id: String { account }
}

extension MailboxModel {
    /// The question `id` is paused for; `nil` once nothing waits.
    func serverChangeReview(_ id: String) -> ServerChangeReview? {
        guard let change = app?.pendingServerChange(accountId: id) else { return nil }
        return ServerChangeReview(account: id, address: signInName(id), change: change)
    }

    /// Allows the new server. Blocking in the core, which connects it first; on a failure nothing
    /// changed and the use stays paused.
    func acceptServerChange(_ id: String) async -> Bool {
        guard let app else { return false }
        do {
            try await Task.detached(priority: .userInitiated) {
                try app.acceptServerChange(accountId: id)
            }.value
            refreshAccountsSnapshot()
            return true
        } catch {
            logAppleLifecycle("accounts: allowing a new server failed (\(coreMessage(error)))")
            return false
        }
    }
}

extension View {
    /// Presents the server-change question `review` holds, and clears it once answered.
    func serverChangeReview(
        _ model: MailboxModel, _ review: Binding<ServerChangeReview?>
    ) -> some View {
        sheet(item: review) { shown in
            ServerChangeDialog(model: model, review: shown) { review.wrappedValue = nil }
        }
    }
}

struct ServerChangeDialog: View {
    var model: MailboxModel
    let review: ServerChangeReview
    let close: () -> Void

    @State private var allowing = false
    @State private var failed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Scrolls on a phone, where a large text size can outgrow the sheet; a macOS sheet
            // sizes to what it holds.
            #if os(iOS)
                ScrollView { question }
            #else
                question
            #endif
            HStack(spacing: 10) {
                Spacer()
                if allowing {
                    ProgressView().controlSize(.small)
                }
                Button(L10n.server_changed_not_now(), role: .cancel, action: close)
                    .keyboardShortcut(.cancelAction)
                Button(L10n.server_changed_allow(), action: allow)
            }
            .buttonStyle(.bordered)
            #if os(iOS)
                .controlSize(.large)
            #endif
            .disabled(allowing)
        }
        .padding(20)
        // Swiping the sheet away is "Not now", except while the new server is being dialled.
        .interactiveDismissDisabled(allowing)
        // A macOS sheet has no intrinsic width to derive from a wrapping paragraph, so it must be
        // given one; on iOS the sheet is the screen's width and a fixed one would clip.
        #if os(macOS)
            .frame(width: 440)
        #else
            .presentationDetents([.medium, .large])
        #endif
    }

    private var question: some View {
        let change = review.change
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
            if failed {
                Text(L10n.server_changed_failed())
                    .font(.callout)
                    .foregroundStyle(.red)
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func allow() {
        allowing = true
        failed = false
        Task {
            let accepted = await model.acceptServerChange(review.account)
            allowing = false
            if accepted {
                close()
            } else {
                failed = true
            }
        }
    }
}
