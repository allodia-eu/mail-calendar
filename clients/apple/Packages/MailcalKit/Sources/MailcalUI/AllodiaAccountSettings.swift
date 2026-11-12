// Settings → Allodia account: the whole category, and the only place the app draws one. Its
// Android, Windows and Linux twins are SettingsAllodia.kt, SettingsDialog.Allodia.cs and
// settings/allodia.rs, keep the states and the wording in step.
//
// A category of its own rather than a card under Accounts, because an Allodia account is not a mail
// account: no mailbox, no switcher entry, and a token issued for it cannot touch anyone's mail. The
// setup wizard never offers it.
//
// The CATEGORY is dropped when `allodiaSignInAvailable()` says this build carries no route, so a
// build from source has no such screen at all, absent, never present-and-broken.

import MailcalBindings
import SwiftUI

/// Drawn from the core's card (`allodiaCard`): signed out → sign in or create; a sign-in out → a
/// spinner and its Cancel; signed in → who, and what to do next; a sign-in the service refused →
/// whose sign-in ended, and signing in again.
struct AllodiaAccountSettings: View {
    var model: MailboxModel

    var body: some View {
        if let card = model.allodia.card {
            VStack(alignment: .leading, spacing: 12) {
                accountBox(card)
                // A subscription belongs to an account, so there is nothing to say about one
                // before somebody has signed in. Keyed by the account, so whoever signs in next
                // starts with no note about somebody else's purchase.
                if case let .signedIn(account) = card.state {
                    AllodiaSubscriptionSettings(model: model).id(account.email)
                }
            }
        }
    }

    private func accountBox(_ card: AllodiaCard) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                Text(L10n.settings_allodia_heading()).font(.headline)
                Text(L10n.settings_allodia_description())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                content(card)
                if let failure = card.failure {
                    Text(AllodiaCardRules.failure(failure))
                        .font(.caption)
                        .foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }

    @ViewBuilder
    private func content(_ card: AllodiaCard) -> some View {
        switch AllodiaCardRules.face(card) {
        case let .signingIn(browser):
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L10n.settings_allodia_signing_in()).foregroundStyle(.secondary)
                ControlButton(title: L10n.action_cancel(), control: browser.cancel) {
                    model.cancelAllodiaSignIn(browser)
                }
            }
        case .signedOut:
            // Both routes. Someone who has no account and someone returning to one need different
            // pages, and a lone "Sign in" sends the first of them through a form asking for a
            // password they never set.
            HStack(spacing: 12) {
                ControlButton(title: L10n.settings_allodia_sign_in(), control: card.signIn) {
                    model.signInToAllodia()
                }
                ControlButton(title: L10n.settings_allodia_create(), control: card.create) {
                    model.signInToAllodia(create: true)
                }
            }
        case let .signedIn(account): signedIn(account, card)
        case let .ended(account): ended(account, card)
        }
    }

    /// The name is what the person recognises, but the address is what identifies the account, so
    /// it is the address that is always shown, and the name only when the service holds one.
    private func signedIn(_ account: AllodiaAccount, _ card: AllodiaCard) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            if let name = account.name, !name.isEmpty {
                Text(name).font(.callout)
            }
            Text(L10n.settings_allodia_signed_in(email: account.email))
                .font(.callout)
                .foregroundStyle(.secondary)
            // Managing and deleting are the same page, named twice on purpose: an account someone
            // can create has to offer deletion somewhere findable, and "Manage account" is not the
            // word anybody looks for when they want out. The page opens in the same in-app browser
            // tab the sign-in uses, which is what makes it open already signed in.
            Button(L10n.settings_allodia_manage()) { model.openAllodiaAccountPage() }
            Text(L10n.settings_allodia_manage_hint())
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            Button(L10n.settings_allodia_delete()) { model.openAllodiaAccountPage() }
            ControlButton(title: L10n.settings_allodia_sign_out(), control: card.signOut) {
                model.signOutOfAllodia()
            }
        }
    }

    /// The service refused the stored sign-in. Its address stays on screen so the person knows
    /// which account to sign in to again; nothing else here can work until they do.
    private func ended(_ account: AllodiaAccount, _ card: AllodiaCard) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.settings_allodia_session_ended(email: account.email))
                .font(.callout)
                .fixedSize(horizontal: false, vertical: true)
            Text(L10n.settings_allodia_session_ended_hint())
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 12) {
                ControlButton(title: L10n.settings_allodia_reauth_action(), control: card.signIn) {
                    model.signInToAllodia()
                }
                .buttonStyle(.borderedProminent)
                ControlButton(title: L10n.settings_allodia_sign_out(), control: card.signOut) {
                    model.signOutOfAllodia()
                }
            }
        }
    }
}
