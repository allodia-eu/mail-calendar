// Settings → Allodia account → Subscription: what is being charged, by whom, until when, and the
// way to start one. Its Android, Windows and Linux twins do not exist yet; when they do, this is
// the wording to keep in step.
//
// The rules are `purchasing.md`, the contract beside the Allodia Licence, and the core holds every
// one of them. **What this file draws is what the core answered.** Which offers exist and in what
// order is `orderAllodiaOffers`; whether a subscription can be changed at all is `actions`; and a
// store's subscription is changed only at that store, so the button opens `manageUrl` rather than
// doing anything itself.
//
// ⚠️ **A price is drawn, never parsed and never assembled.** Apple requires the store's own
// formatted price be shown, the store's commission is already inside it, and the core carries no
// locale data to format one with.

import MailcalBindings
import StoreKit
import SwiftUI
#if os(iOS)
import UIKit
#endif

extension AllodiaBiller {
    /// What this biller is called on screen.
    ///
    /// **Allodia, never the payment processor behind it.** Naming a processor somebody has never
    /// heard of, inside a message about being charged twice, is how a correct warning reads as a
    /// scam. The bare company name is right here, where the sentence is about who is taking the
    /// money rather than about the app.
    var displayName: String {
        switch self {
        case .allodia: return "Allodia"
        case .apple: return "Apple"
        case .google: return "Google Play"
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        case let .unknown(label): return label
        }
    }
}

extension AllodiaPlan {
    /// Which period, for a button and for a line about one.
    var displayName: String {
        switch self {
        case .yearly: return L10n.settings_subscription_yearly()
        case .monthly: return L10n.settings_subscription_monthly()
        }
    }
}

/// Everyone charging for this account right now, in a stable order: Allodia's own billing first,
/// then each store in the order the service listed them.
///
/// Pure, so the sentence a screen says about being charged twice is unit-testable.
func allodiaBillers(of subscription: AllodiaSubscription) -> [AllodiaBiller] {
    var billers: [AllodiaBiller] = subscription.own == nil ? [] : [.allodia]
    billers.append(contentsOf: subscription.stores.map { store in
        switch store.source {
        case .apple: return .apple
        case .google: return .google
        // The service never reports its own billing as a store, so this arm is unreachable rather
        // than meaningful. Saying "Allodia" is still the right answer if it ever is reached.
        case .allodia: return .allodia
        }
    })
    return billers
}

/// Whether anything will charge again, which decides between "renews on" and "runs until".
///
/// A store subscription says so itself; Allodia's own says so by still having a next payment.
/// Pure, and deliberately not a reading of `entitled`, which answers a different question.
func allodiaWillRenew(_ subscription: AllodiaSubscription) -> Bool {
    if subscription.stores.contains(where: \.autoRenewing) { return true }
    guard let own = subscription.own else { return false }
    return own.status == .active && own.nextPaymentDate != nil
}

/// Whether a charge has failed and is being retried. **Access continues**, so this is never drawn
/// as a lapse.
func allodiaInGrace(_ subscription: AllodiaSubscription) -> Bool {
    subscription.stores.contains { $0.status == .grace } || subscription.own?.status == .pastDue
}

/// Settings → Allodia account → Subscription. Drawn only while somebody is signed in: a
/// subscription belongs to an account, and there is nothing to say about one that does not exist.
struct AllodiaSubscriptionSettings: View {
    var model: MailboxModel

    @Environment(\.openURL) private var openURL
    @State private var state: AllodiaSubscriptionState = .checking
    /// Which period has a purchase sheet up, so its button alone shows the spinner.
    @State private var buying: AllodiaPlan?
    /// The last thing worth saying about an attempt: a pending approval, or a failure in the
    /// store's own words. Cleared when the next attempt starts.
    @State private var note: String?

    var body: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                Text(L10n.settings_subscription_heading()).font(.headline)
                content
                if let note {
                    Text(note)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .task { state = await model.allodiaSubscriptionState() }
    }

    @ViewBuilder
    private var content: some View {
        switch state {
        case .checking:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L10n.settings_subscription_checking()).foregroundStyle(.secondary)
            }
        case .unavailable:
            // Deliberately quiet. A read that did not arrive costs a sentence, never access:
            // whether a capability is on is `allodiaEntitlement`'s answer, and it is local.
            Text(L10n.settings_subscription_unavailable())
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        case let .loaded(view):
            loaded(view)
        }
    }

    @ViewBuilder
    private func loaded(_ view: AllodiaSubscriptionView) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            if view.subscription.entitled {
                active(view.subscription)
            } else {
                Text(L10n.settings_subscription_free())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                offers(view.offers)
            }
            // ⚠️ Money was taken and nothing has been granted. A screen that stays silent here
            // leaves somebody with no way to find that out.
            if view.anythingStuck {
                Text(L10n.settings_subscription_stuck())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    /// What a paid account says: who is charging, until when, and where to change it.
    @ViewBuilder
    private func active(_ subscription: AllodiaSubscription) -> some View {
        let billers = allodiaBillers(of: subscription)
        if let first = billers.first {
            Text(L10n.settings_subscription_billed_by(biller: first.displayName))
                .font(.callout)
        }
        if let end = subscription.currentPeriodEnd.flatMap(allodiaDate) {
            Text(
                allodiaWillRenew(subscription)
                    ? L10n.settings_subscription_renews(date: end)
                    : L10n.settings_subscription_ends(date: end)
            )
            .font(.callout)
            .foregroundStyle(.secondary)
        }
        if allodiaInGrace(subscription), let first = billers.first {
            // Not a lapse, and never drawn as one: the store is retrying and access continues.
            Text(L10n.settings_subscription_grace(biller: first.displayName))
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        // Reported, never resolved. Cancelling one of them without asking is a decision about
        // somebody else's money, so each exit is offered and none is taken.
        if !subscription.duplicateBilling.isEmpty {
            Text(
                L10n.settings_subscription_duplicate(
                    billers: subscription.duplicateBilling.map(\.displayName)
                        .joined(separator: ", ")
                )
            )
            .font(.caption)
            .foregroundStyle(.orange)
            .fixedSize(horizontal: false, vertical: true)
        }
        // A store's subscription is the store's to change, so this opens its page rather than
        // offering a cancel button that would have nothing to call. Review-blocking on both
        // stores, which is why it is drawn for every store subscription rather than for a
        // recognised one.
        ForEach(subscription.stores.indices, id: \.self) { index in
            let store = subscription.stores[index]
            let biller: AllodiaBiller = store.source == .google ? .google : .apple
            Button(L10n.settings_subscription_manage(biller: biller.displayName)) {
                manage(store)
            }
        }
    }

    /// The plans that can be bought here, in the core's order.
    @ViewBuilder
    private func offers(_ available: [AllodiaOffer]) -> some View {
        ForEach(available.indices, id: \.self) { index in
            let offer = available[index]
            Button {
                buy(offer.plan)
            } label: {
                if buying == offer.plan {
                    ProgressView().controlSize(.small)
                } else {
                    Text(L10n.settings_subscription_buy(price: offer.displayPrice))
                }
            }
            .disabled(buying != nil)
            // The period and what renewing means, beside the button and not a tap away: both
            // stores make this review-blocking, and somebody agreeing to a recurring charge is
            // owed it whether or not they are.
            Text("\(offer.plan.displayName). \(L10n.settings_subscription_terms())")
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// Opens the store's own subscription page, which is the only thing that can change a
    /// subscription the store is billing.
    ///
    /// iOS has a sheet for this and Apple's review asks for it by name; macOS has no equivalent,
    /// so there the service's `manageUrl` is opened instead. A store that is not Apple is a URL on
    /// both, which is what Play's deep link already is.
    private func manage(_ store: AllodiaStoreSubscription) {
        #if os(iOS)
        if store.source == .apple {
            Task {
                guard
                    let scene = UIApplication.shared.connectedScenes
                        .first(where: { $0.activationState == .foregroundActive })
                        as? UIWindowScene
                else { return }
                try? await AppStore.showManageSubscriptions(in: scene)
            }
            return
        }
        #endif
        if let url = URL(string: store.manageUrl) { openURL(url) }
    }

    private func buy(_ plan: AllodiaPlan) {
        Task {
            note = nil
            buying = plan
            let outcome = await model.buyAllodiaSubscription(plan)
            buying = nil
            switch outcome {
            case .bought:
                // Re-read rather than assume: what was bought is granted only once the account
                // service has attached it, and this read is what says whether it has.
                state = await model.allodiaSubscriptionState()
            case .awaitingApproval:
                note = L10n.settings_subscription_pending()
            // A dismissed sheet is not a failure. Say nothing and leave the button as it was.
            case .cancelled: break
            case let .failed(error):
                note = L10n.settings_subscription_failed(error: error)
            }
        }
    }
}

/// An ISO 8601 date from the account service as a plain calendar date.
///
/// Day precision on purpose: a renewal is a date somebody's bank statement will agree with, and an
/// hour and minute in this sentence would invite a comparison with a clock that means nothing here.
@MainActor func allodiaDate(_ raw: String) -> String? {
    guard let date = parseUtcInstant(raw) else {
        // A bare date, or a naive one. Both are already the answer.
        return raw.isEmpty ? nil : String(raw.prefix(10))
    }
    return allodiaDateFormatter.string(from: date)
}

/// Built once: `DateFormatter` loads ICU locale data on construction, which the timestamp helpers
/// next door carry the same warning about.
@MainActor private let allodiaDateFormatter: DateFormatter = {
    let output = DateFormatter()
    output.dateStyle = .long
    output.timeStyle = .none
    return output
}()
