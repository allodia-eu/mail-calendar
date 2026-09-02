// The two surfaces of "make this your default mail app": the one-time offer, and the Settings row
// that is the way back from it. The platform call is DefaultMailApp.swift; when to ask is the
// shared core's (docs/os-integration.md).

import SwiftUI

/// The one-time offer: when it is raised, and the alert it is raised as.
///
/// Both halves live here rather than in the shell, which is at its line cap and has no business
/// knowing the rule. Raising is asked of the **core** and never decided here: it answers no before
/// the first account exists, when the app is already the default, and once the offer has been put,
/// so no client keeps a "have we asked?" flag of its own that could disagree with Settings
/// (docs/os-integration.md).
///
/// Bound to the account count for the first of those reasons: on a first run this fires again the
/// moment setup finishes, which is the earliest honest moment to ask. `initial: true` covers the
/// ordinary launch, where the count never changes because the accounts were already there.
///
/// An `alert` rather than a `confirmationDialog` for the reason the remove-account prompt gives:
/// iPadOS presents the latter as a popover, and a popover drops the `.cancel`-role button, leaving
/// one button and no way out.
///
/// Both buttons end the offer. Dismissing without choosing is recorded as declined, because a
/// question closed without an answer has still been answered, and asking again is how a prompt
/// becomes nagging.
struct DefaultMailAppOfferDialog: ViewModifier {
    @Bindable var model: MailboxModel

    func body(content: Content) -> some View {
        content
        .onChange(of: model.accounts.count, initial: true) { _, _ in
            model.offerDefaultMailAppIfDue()
        }
        .alert(
            L10n.default_mail_app_offer_title(),
            isPresented: $model.offeringDefaultMailApp
        ) {
            Button(L10n.default_mail_app_offer_accept()) { model.acceptDefaultMailApp() }
            Button(L10n.default_mail_app_offer_decline(), role: .cancel) {
                model.declineDefaultMailApp()
            }
        } message: {
            Text(L10n.default_mail_app_offer_message())
        }
    }
}

/// The Settings → General row. Drawn only where the build can act, so it never offers an action
/// that would silently fail; the caller checks that.
struct DefaultMailAppRow: View {
    @Bindable var model: MailboxModel

    var body: some View {
        HStack(spacing: 12) {
            Button(action: { model.acceptDefaultMailApp() }) {
                Text(
                    DefaultMailApp.support == .openSettings
                        ? L10n.settings_default_mail_app_open_settings()
                        : L10n.settings_default_mail_app_action()
                )
            }
            // Said only when they turned it down: "you have not been asked yet" is not something
            // to tell someone, and "you already did" is what the OS's own settings show.
            if model.defaultMailAppOffer == false {
                Text(L10n.settings_default_mail_app_declined())
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
        }
    }
}
