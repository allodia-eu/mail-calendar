import MailcalBindings
import SwiftUI

/// The sheet shown when a message went out but its copy never reached the account's Sent folder.
///
/// Loud on purpose. A Sent copy is how a person checks that a message really left, so losing one
/// silently is worse than most failures that *do* interrupt, and nothing later recovers it, since
/// there is no copy on the server for a sync to find. The moment it happens is the only moment to
/// say so.
///
/// The copy says what is true and no more: the message **was sent**, and the recipients have it.
/// Wording this as a failed send would make the user's next move "send it again", which is exactly
/// the wrong one, and the reason filing is a separate operation from delivering in the first
/// place.
struct UnfiledCopyPromptView: View {
    let unfiled: UnfiledCopy
    /// File the copy, sends nothing, and is safe to press twice.
    let onRetry: () -> Void
    /// Accept the missing copy and close.
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Label(L10n.unfiled_copy_title(), systemImage: "exclamationmark.circle.fill")
                .font(.headline)
                .labelStyle(.titleAndIcon)
                .foregroundStyle(.orange)
            // The subject is the user's own text, but it is still rendered as text, never markup.
            Text(L10n.unfiled_copy_body(subject: unfiled.subject))
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                Button(L10n.unfiled_copy_dismiss(), action: onDismiss)
                    .disabled(unfiled.retrying)
                Spacer()
                Button(action: onRetry) {
                    if unfiled.retrying {
                        ProgressView().controlSize(.small)
                    } else {
                        Text(L10n.unfiled_copy_retry())
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(unfiled.retrying)
            }
        }
        .padding(20)
        .frame(minWidth: 360)
    }
}
