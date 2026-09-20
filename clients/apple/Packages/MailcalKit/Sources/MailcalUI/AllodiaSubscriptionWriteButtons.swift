// The three things somebody may do to the subscription Allodia bills directly: the buttons, the
// two confirmations, and the sentence each ending earns.
//
// The decisions are all next door in `AllodiaSubscriptionWrites.swift`, which is pure and tested.
// What is here is only their arrangement, and the two confirmations, which are drawn **inline**
// rather than as an alert: Settings is already a sheet on iOS, and a confirmation that costs
// money reads better where the subscription it is about is still visible.

import MailcalBindings
import SwiftUI

/// Which confirmation is open, if any.
private enum AllodiaPendingWrite {
    case none
    case cancel
    case switchInterval(AllodiaPlan)
}

/// What may be done to Allodia's own subscription, drawn only where `actions` permits it.
///
/// ⚠️ **A store's subscription gets none of this.** It is changed at that store, which is what the
/// manage button above offers; the service refuses all three here and says so through `actions`.
struct AllodiaSubscriptionWriteButtons: View {
    let subscription: AllodiaSubscription
    let busy: Bool
    /// Runs one write and re-reads afterwards. **What the write did is the next read's answer**,
    /// never this one's: the service recomputes every biller, and a screen editing its own copy
    /// would disagree with it.
    let run: (@escaping () async -> AllodiaWriteResult) -> Void

    var model: MailboxModel

    @State private var pending: AllodiaPendingWrite = .none

    var body: some View {
        let target = allodiaSwitchTarget(subscription)
        if let target {
            Button(label(for: target)) { pending = .switchInterval(target) }
                .disabled(busy)
        }
        if allodiaOffersRestart(subscription) {
            // No confirmation: starting again is what somebody came here to do, and it charges
            // nothing today that the cancellation had not already left running.
            Button(L10n.settings_subscription_resubscribe()) {
                run { await model.resubscribeToAllodia() }
            }
            .disabled(busy)
        }
        if subscription.actions.canCancel {
            Button(L10n.settings_subscription_cancel()) { pending = .cancel }
                .disabled(busy)
        }
        confirmation
    }

    @ViewBuilder
    private var confirmation: some View {
        let day = allodiaDate(allodiaWriteDate(subscription)) ?? ""
        switch pending {
        case .none:
            EmptyView()
        case .cancel:
            confirm(
                title: L10n.settings_subscription_cancel_title(),
                // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they are
                // not losing what they have already paid for. A cancellation with no date reads as
                // "it stops now", which is the one thing it does not do.
                body: L10n.settings_subscription_cancel_body(date: day),
                confirm: L10n.settings_subscription_cancel(),
                // **Never "Cancel" for the way out.** On this confirmation that word is the thing
                // being asked about, so the button that does nothing has to say what it keeps.
                dismiss: L10n.settings_subscription_cancel_keep()
            ) { await model.cancelAllodiaSubscription() }
        case let .switchInterval(target):
            confirm(
                title: L10n.settings_subscription_switch_title(),
                // ⚠️ **No price here, deliberately.** What this subscriber is charged is not
                // today's list price, because a price change never reaches somebody who already
                // subscribed. The amount comes back from the switch itself and is said afterwards.
                body: L10n.settings_subscription_switch_body(date: day),
                confirm: L10n.action_update(),
                dismiss: L10n.action_cancel()
            ) { await model.switchAllodiaInterval(to: target) }
        }
    }

    @ViewBuilder
    private func confirm(
        title: String,
        body: String,
        confirm: String,
        dismiss: String,
        write: @escaping () async -> AllodiaWriteResult
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.callout)
            Text(body)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 8) {
                Button(confirm) {
                    pending = .none
                    run(write)
                }
                Button(dismiss) { pending = .none }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
    }

    private func label(for target: AllodiaPlan) -> String {
        switch target {
        case .yearly: return L10n.settings_subscription_switch_yearly()
        case .monthly: return L10n.settings_subscription_switch_monthly()
        }
    }
}

/// The sentence one ending earns, or `nil` for the one that has nothing to say.
///
/// ⚠️ **A refusal is a code and never a message**, which is what `allodiaWriteFailure` already
/// reduced it to. The twins are `SettingsDialog.SubscriptionWrites.cs`,
/// `AllodiaSubscriptionModel.kt` and `allodia_subscription_facts.rs`.
@MainActor func allodiaWriteNote(
    _ result: AllodiaWriteResult,
    currency: String?
) -> String? {
    switch result {
    case let .cancelled(endDate):
        return L10n.settings_subscription_cancelled(date: allodiaDate(endDate) ?? endDate)
    case .reactivated:
        return L10n.settings_subscription_resubscribed()
    case let .switched(change):
        return L10n.settings_subscription_switched(
            date: change.nextPaymentDate.flatMap(allodiaDate) ?? "",
            amount: allodiaMinorUnits(change.amountInCents, currency: currency)
        )
    case .silent:
        return nil
    case let .failed(failure):
        return allodiaRefusalText(failure)
    }
}

/// What each refusal is called on screen.
@MainActor func allodiaRefusalText(_ failure: AllodiaWriteFailure) -> String {
    switch failure {
    case .alreadyCancelled: return L10n.settings_subscription_refused_already_cancelled()
    case .alreadyActive: return L10n.settings_subscription_refused_already_active()
    case .alreadyOnInterval: return L10n.settings_subscription_refused_already_on_interval()
    case .notSwitchable: return L10n.settings_subscription_refused_not_switchable()
    case .notFound: return L10n.settings_subscription_refused_not_found()
    case .unexplained: return L10n.settings_subscription_write_failed()
    }
}
