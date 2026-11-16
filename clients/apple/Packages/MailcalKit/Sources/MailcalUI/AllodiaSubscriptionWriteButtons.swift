// The three things somebody may do to the subscription Allodia bills directly, as the core offers
// them (`AllodiaSubscriptionView`), and the two confirmations. The confirmations are drawn
// **inline** rather than as an alert: Settings is already a sheet on iOS, and a confirmation that
// costs money reads better where the subscription it is about is still visible.
//
// ⚠️ **Apple draws three of the four writes, and never the checkout.** Starting one means sending
// somebody to a payment page outside the app, which needs Apple's external-purchase-link
// entitlement; on this platform buying is StoreKit's (`purchasing.md`).

import MailcalBindings
import SwiftUI

/// Which confirmation is open, if any.
private enum AllodiaPendingWrite {
    case none
    case cancel
    case switchInterval(AllodiaPlan)
}

/// What may be done to Allodia's own subscription, each drawn only while the core offers it. A
/// write's spinner is on the button that asked, since its confirmation closes on the press.
struct AllodiaSubscriptionWriteButtons: View {
    let view: AllodiaSubscriptionView
    var model: MailboxModel

    @State private var pending: AllodiaPendingWrite = .none

    var body: some View {
        if let target = view.`switch` {
            ControlButton(title: target.to.switchLabel, control: target.control) {
                pending = .switchInterval(target.to)
            }
        }
        if let resubscribe = view.resubscribe {
            // No confirmation: starting again is what somebody came here to do, and it charges
            // nothing today that the cancellation had not already left running.
            ControlButton(title: L10n.settings_subscription_resubscribe(), control: resubscribe) {
                model.resubscribeToAllodia()
            }
        }
        if let cancel = view.cancel {
            ControlButton(
                title: L10n.settings_subscription_cancel(), role: .destructive, control: cancel
            ) { pending = .cancel }
        }
        confirmation
    }

    @ViewBuilder
    private var confirmation: some View {
        let day = view.paidThrough.flatMap(allodiaDate) ?? ""
        switch pending {
        case .none:
            EmptyView()
        case .cancel:
            if let control = view.cancel {
                confirm(
                    title: L10n.settings_subscription_cancel_title(),
                    // ⚠️ The date is the whole reassurance: somebody cancelling wants to know they
                    // are not losing what they have already paid for. A cancellation with no date
                    // reads as "it stops now", which is the one thing it does not do.
                    body: L10n.settings_subscription_cancel_body(date: day),
                    confirm: L10n.settings_subscription_cancel(),
                    role: .destructive,
                    // **Never "Cancel" for the way out.** On this confirmation that word is the
                    // thing being asked about, so the button that does nothing says what it keeps.
                    dismiss: L10n.settings_subscription_cancel_keep(),
                    control: control
                ) { model.cancelAllodiaSubscription() }
            }
        case let .switchInterval(target):
            if let control = view.`switch`?.control {
                confirm(
                    title: L10n.settings_subscription_switch_title(),
                    // ⚠️ **No price here, deliberately.** What this subscriber is charged is not
                    // today's list price, because a price change never reaches somebody who
                    // already subscribed. The amount comes back from the switch and is said then.
                    body: L10n.settings_subscription_switch_body(date: day),
                    confirm: L10n.action_update(),
                    role: nil,
                    dismiss: L10n.action_cancel(),
                    control: control
                ) { model.switchAllodiaInterval(to: target) }
            }
        }
    }

    private func confirm(
        title: String,
        body: String,
        confirm: String,
        role: ButtonRole?,
        dismiss: String,
        control: ControlState,
        write: @escaping () -> Void
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.callout)
            Text(body)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 8) {
                ControlButton(title: confirm, role: role, control: control) {
                    pending = .none
                    write()
                }
                Button(dismiss) { pending = .none }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
    }
}
