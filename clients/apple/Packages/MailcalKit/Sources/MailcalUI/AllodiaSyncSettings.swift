// Settings → Accounts: what the person's other devices have to say, above their own mail accounts.
// Its Android, Windows and Linux twins draw the same three things in the same order, keep the
// states and the wording in step.
//
// It sits in Accounts rather than in the Allodia-account category because what it is about is mail
// accounts: one arriving is an account to set up, and that is where somebody looks for it.

import MailcalBindings
import SwiftUI

/// Offers to set up, accounts that moved elsewhere, and accounts removed elsewhere.
///
/// Draws nothing at all when there is nothing to say, including before the first pass has run,
/// which must not look like a pass that found nothing.
struct AllodiaSyncSettings: View {
    var model: MailboxModel
    /// Opens the setup flow with the address already typed.
    /// Sets an offered account up on the route its record names, not the address alone.
    let setUp: (AllodiaAccountOffer) -> Void

    var body: some View {
        let state = model.allodiaSync
        if state.checking || state.failure != nil || state.hasSomethingToSay {
            GroupBox {
                VStack(alignment: .leading, spacing: 10) {
                    Text(L10n.settings_allodia_sync_heading()).font(.headline)
                    Text(L10n.settings_allodia_sync_description())
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    content(state)
                    if state.failure != nil {
                        failureView(state.health)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(6)
            }
        }
    }

    /// What a failed pass is allowed to put on screen.
    ///
    /// The core's typed answer decides, never the failure's text. A grant that predates a
    /// permission and one the service revoked are different sentences with different remedies, and
    /// everything else says nothing about the sign-in at all, so it gets one plain line, and the
    /// detail stays in the diagnostic log. There is no longer a path from an error's text to a
    /// screen, which is what put `invalid_scope, unable to issue scope mailcal:accounts:read` in
    /// front of somebody.
    @ViewBuilder
    private func failureView(_ health: AllodiaGrantHealth) -> some View {
        switch health {
        case .needsReauth:
            // An offer, not an error: they are signed in and one feature is asleep.
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.settings_allodia_reauth()).font(.callout.weight(.semibold))
                Text(L10n.settings_allodia_reauth_hint())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                Button(L10n.settings_allodia_reauth_action()) { signInAgain() }
            }
        case .signedOut:
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.settings_allodia_signed_out_elsewhere()).font(.callout.weight(.semibold))
                Text(L10n.settings_allodia_signed_out_elsewhere_hint())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                Button(L10n.settings_allodia_sign_in()) { signInAgain() }
            }
        case .ok:
            Text(L10n.settings_allodia_sync_unavailable())
                .font(.caption)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// The ordinary sign-in again, which is the whole remedy for both states: it asks for the full
    /// current scope set every time, so it widens a grant that predates one and replaces a grant
    /// the service refused.
    private func signInAgain() {
        Task {
            if case .signedIn = await model.signInToAllodia() {
                await model.syncAllodiaAccounts()
            }
        }
    }

    @ViewBuilder
    private func content(_ state: AllodiaSyncState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            if state.checking {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L10n.settings_allodia_sync_checking()).foregroundStyle(.secondary)
                }
            }
            if let report = state.report {
                ForEach(report.offers, id: \.id) { offer in
                    HStack {
                        Text(offer.email)
                        Spacer()
                        Button(L10n.settings_allodia_sync_set_up()) { setUp(offer) }
                    }
                }
                // Both of these are questions, and the only answer this device can act on today is
                // "keep what I have". Applying the other side's settings needs a path for editing
                // a connected account's server details, which does not exist yet.
                ForEach(report.changedElsewhere, id: \.accountId) { change in
                    question(L10n.settings_allodia_changed_elsewhere(email: change.email), change)
                }
                ForEach(report.removedElsewhere, id: \.accountId) { change in
                    question(L10n.settings_allodia_removed_elsewhere(email: change.email), change)
                }
            }
        }
    }

    @ViewBuilder
    private func question(_ text: String, _ change: AllodiaAccountChange) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(text).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            // "Keep what I have" is Paused: the other devices keep the account, and this one
            // stops exchanging changes about it, which is exactly what the question asked.
            Button(L10n.settings_allodia_keep_local()) {
                Task { await model.setAllodiaAccountSyncMode(change.accountId, .paused) }
            }
        }
    }
}
