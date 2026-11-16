// Settings → Accounts: what the person's other devices have to say, above their own mail accounts.
// Its Android, Windows and Linux twins draw the same three things in the same order, keep the
// states and the wording in step.
//
// It sits in Accounts rather than in the Allodia-account category because what it is about is mail
// accounts: one arriving is an account to set up, and that is where somebody looks for it.

import MailcalBindings
import SwiftUI

/// Offers to set up, accounts that moved elsewhere, and accounts removed elsewhere, as the core's
/// `allodiaDevices` has them.
///
/// Draws nothing before the first pass has answered, and nothing once one answered with nothing:
/// a heading over an empty list claims something this screen has no reason to say.
struct AllodiaSyncSettings: View {
    var model: MailboxModel
    /// Sets an offered account up on the route its record names, not the address alone.
    let setUp: (AllodiaAccountOffer) -> Void

    var body: some View {
        if let devices = model.allodiaDevices, !devices.nothingFound {
            GroupBox {
                VStack(alignment: .leading, spacing: 10) {
                    Text(L10n.settings_allodia_sync_heading()).font(.headline)
                    Text(L10n.settings_allodia_sync_description())
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    content(devices)
                    if let failure = devices.failure {
                        failureView(failure)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(6)
            }
        }
    }

    /// What a failed pass is allowed to put on screen: the core's typed answer, never a failure's
    /// text. A grant that predates a permission and one the service revoked are different
    /// sentences with different remedies; everything else gets one plain line, and the detail
    /// stays in the diagnostic log.
    @ViewBuilder
    private func failureView(_ failure: AllodiaSyncFailure) -> some View {
        switch failure {
        case .needsReauth:
            // An offer, not an error: they are signed in and one feature is asleep.
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.settings_allodia_reauth()).font(.callout.weight(.semibold))
                Text(L10n.settings_allodia_reauth_hint())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                signInAgain(L10n.settings_allodia_reauth_action())
            }
        case .signedOut:
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.settings_allodia_signed_out_elsewhere()).font(.callout.weight(.semibold))
                Text(L10n.settings_allodia_signed_out_elsewhere_hint())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                signInAgain(L10n.settings_allodia_sign_in())
            }
        case .unavailable:
            Text(L10n.settings_allodia_sync_unavailable())
                .font(.caption)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// The ordinary sign-in again, which is the whole remedy for both states: it asks for the full
    /// current scope set every time, so it widens a grant that predates one and replaces a grant
    /// the service refused. The core runs the pass once it is stored.
    private func signInAgain(_ title: String) -> some View {
        ControlButton(title: title, control: model.allodiaSignInControl) {
            model.signInToAllodia()
        }
    }

    @ViewBuilder
    private func content(_ devices: AllodiaDevices) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            if devices.checking {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L10n.settings_allodia_sync_checking()).foregroundStyle(.secondary)
                }
            }
            ForEach(devices.offers, id: \.id) { offer in
                HStack {
                    Text(offer.email)
                    Spacer()
                    Button(L10n.settings_allodia_sync_set_up()) { setUp(offer) }
                }
            }
            // The only answer this device can act on is "keep what I have". Applying the other
            // side's settings needs a path for editing a connected account's server details, which
            // does not exist yet.
            ForEach(devices.questions, id: \.accountId) { question in
                questionRow(question)
            }
        }
    }

    private func questionRow(_ question: AllodiaDevicesQuestion) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(AccountSettingsLabels.elsewhere(question))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            // "Keep what I have" pauses the account's sharing: the other devices keep it, and
            // this one stops exchanging changes about it, which is what the question asked.
            ControlButton(title: L10n.settings_allodia_keep_local(), control: question.keep) {
                model.changeAccount(.setSharing(account: question.accountId, mode: .paused))
            }
            if let failed = question.failed {
                SharingFailedLine(failure: failed)
            }
        }
    }
}

/// Why a sharing change did not take, as a plain line under what it was asked from.
struct SharingFailedLine: View {
    let failure: AllodiaSyncFailure

    var body: some View {
        Text(AccountSettingsLabels.sharingFailed(failure))
            .font(.caption)
            .foregroundStyle(.red)
            .fixedSize(horizontal: false, vertical: true)
    }
}
