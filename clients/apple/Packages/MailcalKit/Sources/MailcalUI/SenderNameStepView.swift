// The "your name" step: the one question asked after an account connects (docs/sending.md).
//
// After, not before. The screen that adds an account is the address field and nothing else
// (docs/onboarding.md), and by this point the account exists, so the provider can be asked what
// it already calls this person. On a Microsoft or Gmail account that means the field arrives
// filled in and the step is a confirmation.

import MailcalBindings
import SwiftUI

/// The account whose name step is open. A type rather than a bare `String` so `.sheet(item:)`
/// can key on it; the id *is* the account id, so re-presenting for the same account is a no-op.
struct SenderNamePrompt: Identifiable, Equatable {
    let id: String
}

/// Asks what to call the sender of this account's mail. Dismissed either way: skipping leaves
/// the account sending as a bare address, which is a state the app has to be correct in.
struct SenderNameStepView: View {
    var model: MailboxModel
    let account: String
    let done: () -> Void

    @State private var name: String = ""
    /// True until the suggestion has been asked for. The lookup talks to the provider, so the
    /// field stays disabled rather than accepting typing that is about to be overwritten.
    @State private var loading = true
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(L10n.setup_sender_name_title()).font(.title2).bold()
            Text(L10n.setup_sender_name_description())
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            TextField(L10n.setup_sender_name_field(), text: $name)
                .textFieldStyle(.roundedBorder)
                .disabled(loading)
                .focused($focused)
                .onSubmit(save)
                #if os(iOS)
                    .textInputAutocapitalization(.words)
                    .autocorrectionDisabled()
                #endif

            HStack {
                Button(L10n.setup_sender_name_skip(), action: done)
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                Spacer()
                Button(L10n.setup_sender_name_continue(), action: save)
                    .keyboardShortcut(.defaultAction)
                    .disabled(loading)
            }
        }
        .padding(24)
        .frame(minWidth: 320)
        .task {
            // Read the handle on the main actor, then leave it: the lookup is a provider round
            // trip and would freeze the sheet, and on an account with no server-side name it
            // simply answers empty.
            guard let app = model.app else {
                loading = false
                focused = true
                return
            }
            name = await Task.detached(priority: .userInitiated) { [account] in
                app.suggestedSenderName(account: account)
            }.value
            loading = false
            focused = true
        }
    }

    /// Stores the name and closes the step. An empty field is a real answer, and storing it
    /// clears rather than fails: the user chose to send as their address.
    private func save() {
        model.setAccountSenderName(account, name)
        done()
    }
}
