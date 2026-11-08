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

/// Signed out → sign in or create; signing in → a spinner; signed in → who, and what to do next;
/// a sign-in the service refused → whose sign-in ended, and signing in again.
struct AllodiaAccountSettings: View {
    var model: MailboxModel

    /// Who is signed in. Seeded from the core on appear and re-read after every sign-in or
    /// sign-out, which is cheap: the answer is local and never asks the service.
    @State private var account: AllodiaAccount?
    @State private var signingIn = false
    /// The last failure, in the service's own words. Cleared when a new attempt starts.
    @State private var failure: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            accountBox
            // A subscription belongs to an account, so there is nothing to say about one before
            // somebody has signed in. Keyed on the account so signing in or out rebuilds it and
            // its read runs again for whoever is signed in now.
            if let account, !ended {
                AllodiaSubscriptionSettings(model: model).id(account.email)
            }
        }
    }

    private var accountBox: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                Text(L10n.settings_allodia_heading()).font(.headline)
                Text(L10n.settings_allodia_description())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                content
                if let failure {
                    Text(L10n.settings_allodia_failed(error: failure))
                        .font(.caption)
                        .foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .task { account = model.currentAllodiaAccount() }
    }

    @ViewBuilder
    private var content: some View {
        if signingIn {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L10n.settings_allodia_signing_in()).foregroundStyle(.secondary)
            }
        } else if let account, ended {
            ended(account)
        } else if let account {
            // The name is what the person recognises, but the address is what identifies the
            // account, so it is the address that is always shown, and the name only when the
            // service holds one.
            VStack(alignment: .leading, spacing: 6) {
                if let name = account.name, !name.isEmpty {
                    Text(name).font(.callout)
                }
                Text(L10n.settings_allodia_signed_in(email: account.email))
                    .font(.callout)
                    .foregroundStyle(.secondary)
                // Managing and deleting are the same page, named twice on purpose: an account
                // someone can create has to offer deletion somewhere findable, and "Manage
                // account" is not the word anybody looks for when they want out.
                Button(L10n.settings_allodia_manage()) { manage() }
                Text(L10n.settings_allodia_manage_hint())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                Button(L10n.settings_allodia_delete()) { manage() }
                Button(L10n.settings_allodia_sign_out()) { signOut() }
            }
        } else {
            // Both routes. Someone who has no account and someone returning to one need different
            // pages, and a lone "Sign in" sends the first of them through a form asking for a
            // password they never set.
            HStack(spacing: 12) {
                Button(L10n.settings_allodia_sign_in()) { signIn() }
                Button(L10n.settings_allodia_create()) { create() }
            }
        }
    }

    /// The service refused the stored sign-in. Its address stays on screen so the person knows
    /// which account to sign in to again; nothing else here can work until they do.
    private func ended(_ account: AllodiaAccount) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.settings_allodia_session_ended(email: account.email))
                .font(.callout)
                .fixedSize(horizontal: false, vertical: true)
            Text(L10n.settings_allodia_session_ended_hint())
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 12) {
                Button(L10n.settings_allodia_reauth_action()) { signIn() }
                    .buttonStyle(.borderedProminent)
                Button(L10n.settings_allodia_sign_out()) { signOut() }
            }
        }
    }

    /// Whether the service refused the sign-in: drawn while the Allodia notice is in force, so the
    /// card and the notice cannot disagree.
    private var ended: Bool { NoticeRules.allodiaSignInEnded(model.notices ?? []) }

    private func signIn() { begin(create: false) }

    /// Creating an account: the same flow asking the service for its sign-up page instead.
    private func create() { begin(create: true) }

    /// Opens the page where someone changes their details or deletes the account, in the same
    /// in-app browser tab the sign-in uses, which is what makes it open already signed in.
    private func manage() {
        model.openAllodiaAccountPage()
    }

    private func begin(create: Bool) {
        Task {
            failure = nil
            signingIn = true
            let outcome = await model.signInToAllodia(create: create)
            signingIn = false
            switch outcome {
            case let .signedIn(signedIn):
                account = signedIn
                // The first thing a new sign-in is for: this device's accounts go up, and whatever
                // the person's other devices hold comes back.
                await model.syncAllodiaAccounts()
            // A dismissed browser is not a failure, say nothing and leave the button as it was.
            case .cancelled: break
            case let .failed(error): failure = error
            }
        }
    }

    /// The account is forgotten in memory whatever the store does, so re-read rather than assuming:
    /// a delete that failed leaves the app signed out and says why.
    private func signOut() {
        failure = model.signOutOfAllodia()
        account = model.currentAllodiaAccount()
        // Nothing left to say about other devices once this one is signed out of the account that
        // linked them.
        model.allodiaSync = AllodiaSyncState()
    }
}
