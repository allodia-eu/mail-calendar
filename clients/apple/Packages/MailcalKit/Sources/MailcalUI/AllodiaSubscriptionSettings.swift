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
    /// The button that buys this period, priced.
    ///
    /// **The period is on the button, not under it.** It is the thing being chosen, so a row of
    /// buttons that all read "Subscribe" makes the reader pair each one with a line of small print
    /// to find out what it does. The renewal terms stay underneath, because they are the same
    /// sentence for both and belong to the commitment rather than to the choice.
    func buyLabel(price: String) -> String {
        switch self {
        case .yearly: return L10n.settings_subscription_buy_yearly(price: price)
        case .monthly: return L10n.settings_subscription_buy_monthly(price: price)
        }
    }
}

/// Everyone charging for this account right now, in a stable order: Allodia's own billing first,
/// then each store in the order the service listed them.
///
/// ⚠️ **"Right now" is the whole of it, and reading every store the service listed was a bug.** The
/// list keeps a subscription after it ends, so an account that bought at one store and later at
/// another carries both, and naming the first named the dead one: an Android device billed by
/// Google Play was told it was billed by Apple, whose sandbox subscription had run out hours
/// earlier.
///
/// Whether a store still has something to manage is a different question, answered by
/// `allodiaStoreCanBeManaged`, and the two part company on the states that matter most.
///
/// Pure, so the sentence a screen says about being charged twice is unit-testable.
func allodiaBillers(of subscription: AllodiaSubscription) -> [AllodiaBiller] {
    var billers: [AllodiaBiller] = []
    if let own = subscription.own, own.status != .pendingFirstPayment {
        billers.append(.allodia)
    }
    billers.append(contentsOf: subscription.stores.filter { allodiaStoreIsBilling($0.status) }
        .map { store in
            switch store.source {
            case .apple: return .apple
            case .google: return .google
            // The service never reports its own billing as a store, so this arm is unreachable
            // rather than meaningful. Saying "Allodia" is still the right answer if it is reached.
            case .allodia: return .allodia
            }
        })
    return billers
}

/// Whether a store subscription is one somebody is still on: renewing, being retried, or cancelled
/// and running out the period already paid for.
///
/// The rest grant nothing, and `AllodiaStoreStatus` is deliberately read arm by arm rather than by
/// exclusion: a status this build does not know is **never read as permission**, which is the rule
/// the core states about the same enum.
func allodiaStoreIsBilling(_ status: AllodiaStoreStatus) -> Bool {
    switch status {
    case .active, .grace, .cancelled: return true
    case .onHold, .paused, .expired, .revoked: return false
    case .unknown: return false
    }
}

/// Whether the store still has something for this person to do about this subscription.
///
/// ⚠️ **Not the same question as who is billing them, and the two answers differ on exactly the
/// states somebody needs most.** On hold and paused grant nothing, so neither may claim the "billed
/// by" line, and both are fixed only at the store: a card that failed is replaced there and a pause
/// is lifted there. Dropping their button strands the person it matters to.
///
/// Expired and revoked are the other way about. Nothing is left to manage, and offering the route
/// anyway walks somebody into the store's own resubscribe button while another source is already
/// charging them, which is the duplicate billing this contract warns about rather than causes.
func allodiaStoreCanBeManaged(_ status: AllodiaStoreStatus) -> Bool {
    switch status {
    case .active, .grace, .cancelled, .onHold, .paused: return true
    case .expired, .revoked: return false
    // A status this build does not know names no action it could offer.
    case .unknown: return false
    }
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
    /// The wait on that sheet, held so leaving the screen can let go of it.
    ///
    /// ⚠️ **StoreKit puts no bound on `purchase()`, and a sheet that never answers suspends its
    /// caller for the life of the process.** Without this the screen has no way back: every button
    /// stays disabled behind a spinner that will not stop, and only relaunching the app clears it.
    @State private var purchase: Task<Void, Never>?
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
        .onDisappear {
            // **Letting go of the wait is not abandoning the purchase.** Whatever the store does
            // with it arrives on `Transaction.updates`, which starts a redemption pass of its own,
            // and an unfinished transaction is offered again at every launch until the account
            // service has granted it. So the money is safe whether this screen is watching or not,
            // and closing it is the way out of a sheet that never answered.
            purchase?.cancel()
            purchase = nil
            buying = nil
        }
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
        case .needsReauth:
            // An offer rather than an error: they are signed in and this one read is asleep, and
            // the ordinary sign-in asks for the full current scope set.
            Text(L10n.settings_subscription_reauth())
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            Button(L10n.settings_allodia_reauth_action()) { signInAgain() }
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
                // ⚠️ **A device that cannot name its account does not buy here.** The id a purchase
                // is tagged with is the only way to attribute one whose report never arrives, and
                // it cannot be added afterwards, so a grant stored before it existed is asked to
                // sign in again rather than allowed to make a purchase that can never be repaired.
                // Signing in is all it takes, and it is the same remedy the account-list gap uses.
                if model.currentAllodiaAccount()?.id == nil {
                    Text(L10n.settings_subscription_reauth())
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    Button(L10n.settings_allodia_reauth_action()) { signInAgain() }
                } else {
                    offers(view.offers)
                }
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
        // stores, which is why it is drawn for every store that still has something to do rather
        // than for a recognised one.
        let manageable = subscription.stores.filter { allodiaStoreCanBeManaged($0.status) }
        ForEach(manageable.indices, id: \.self) { index in
            let store = manageable[index]
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
                // The label stays while the spinner runs. A button that becomes a bare spinner
                // stops saying what it is doing, which is the wrong half to drop when what it is
                // doing is charging somebody.
                HStack(spacing: 6) {
                    if buying == offer.plan {
                        ProgressView().controlSize(.small)
                    }
                    Text(offer.plan.buyLabel(price: offer.displayPrice))
                }
            }
            .disabled(buying != nil)
            // What renewing means, beside the button and not a tap away: both stores make this
            // review-blocking, and somebody agreeing to a recurring charge is owed it whether or
            // not they are.
            Text(L10n.settings_subscription_terms())
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

    /// Signs in again, which is what records the account id on a grant stored without one.
    ///
    /// The same sign-in as the account box above, not a special one: there is nothing to migrate,
    /// only a claim this device never asked for and now does.
    private func signInAgain() {
        Task {
            if case .signedIn = await model.signInToAllodia() {
                state = await model.allodiaSubscriptionState()
            }
        }
    }

    private func buy(_ plan: AllodiaPlan) {
        purchase = Task {
            note = nil
            buying = plan
            let outcome = await model.buyAllodiaSubscription(plan)
            // A wait this screen has let go of says nothing further: the state it would report is
            // about a screen that is no longer there, and the pass that outlives it is what
            // records the purchase.
            if Task.isCancelled { return }
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
    guard !raw.isEmpty else { return nil }
    if let date = allodiaInstant(raw) {
        return allodiaDateFormatter.string(from: date)
    }
    // A shape this build cannot read. The leading ten characters are still the right day, so the
    // sentence stays true and only stops being localised.
    return String(raw.prefix(10))
}

/// An ISO 8601 instant in the shapes the **account service** sends.
///
/// ⚠️ **`parseUtcInstant` next door is not enough, and using it here was a bug.** It requires a
/// `Z` suffix because the engine's timestamps carry one; the account service is a different
/// server, sends a numeric offset instead, and every date on this screen therefore fell through to
/// the raw string and was drawn as `2026-09-18` to a Dutch reader who should have seen
/// `18 september 2026`.
private func allodiaInstant(_ raw: String) -> Date? {
    if let date = parseUtcInstant(raw) { return date }
    for parser in allodiaIsoParsers {
        if let date = parser.date(from: raw) { return date }
    }
    return allodiaDayParser.date(from: String(raw.prefix(10)))
}

/// Built once each: `ISO8601DateFormatter` loads ICU data on construction, the same cost the
/// timestamp helpers next door carry a warning about. Fractional seconds are a separate parser
/// rather than a flag, because the option makes a formatter **require** them rather than allow
/// them, so one configuration cannot read both shapes.
private nonisolated(unsafe) let allodiaIsoParsers: [ISO8601DateFormatter] = [
    {
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime]
        return iso
    }(),
    {
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return iso
    }(),
]

/// A date with no time at all, which is what the service sends for a period that ends on a day
/// rather than at an instant. POSIX locale: this parses the wire, it never formats for a reader.
private let allodiaDayParser: DateFormatter = {
    let parser = DateFormatter()
    parser.locale = Locale(identifier: "en_US_POSIX")
    parser.timeZone = TimeZone(secondsFromGMT: 0)
    parser.dateFormat = "yyyy-MM-dd"
    return parser
}()

/// Built once: `DateFormatter` loads ICU locale data on construction, which the timestamp helpers
/// next door carry the same warning about.
@MainActor private let allodiaDateFormatter: DateFormatter = {
    let output = DateFormatter()
    output.dateStyle = .long
    output.timeStyle = .none
    return output
}()
