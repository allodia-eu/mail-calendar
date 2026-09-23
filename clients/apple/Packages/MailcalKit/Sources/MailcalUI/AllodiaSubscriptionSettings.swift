// Settings → Allodia account → Subscription: what is being charged, by whom, until when, and the
// way to start one. Its Android, Windows and Linux twins say the same things in the same order;
// keep the wording in step.
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
    /// Whether one of the three writes is in flight, so no second one starts on top of it.
    @State private var writing = false

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
            // ⚠️ The other ending that owes somebody an explanation: what they bought is on a
            // different Allodia account, so it is never coming to this one and no amount of
            // waiting changes that.
            if view.claimedElsewhere {
                Text(L10n.settings_subscription_claimed_elsewhere())
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
        let manageable = allodiaManageableStores(of: subscription)
        ForEach(manageable.indices, id: \.self) { index in
            let store = manageable[index]
            let biller: AllodiaBiller = store.source == .google ? .google : .apple
            Button(L10n.settings_subscription_manage(biller: biller.displayName)) {
                manage(store)
            }
        }
        // Allodia's own subscription is the only one this API can change, and `actions` is what
        // says whether it may. Somebody billed by a store sees none of these, and sees the button
        // above instead.
        AllodiaSubscriptionWriteButtons(
            subscription: subscription,
            busy: writing,
            run: { write in perform(write, currency: subscription.prices.currency) },
            model: model
        )
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
        // Apple's guideline 3.1.2: a screen selling an auto-renewable subscription links the
        // Terms of Use and the privacy policy, beside the purchase rather than elsewhere in the
        // app. Every purchase here is StoreKit's, so the terms are Apple's standard EULA, the one
        // the App Store listing links too. The Android, Windows and Linux twins draw no such line:
        // Apple's EULA governs nothing Google or Allodia bills.
        if !available.isEmpty {
            HStack(spacing: 12) {
                Link(L10n.settings_subscription_eula(), destination: Self.appleStandardEULA)
                if let privacy = URL(string: L10n.welcome_privacy_url()) {
                    Link(L10n.settings_subscription_privacy(), destination: privacy)
                }
            }
            .font(.caption)
        }
    }

    /// Apple's standard licence agreement, which governs an App Store purchase for which the
    /// developer has not uploaded one of its own.
    private static let appleStandardEULA =
        URL(string: "https://www.apple.com/legal/internet-services/itunes/dev/stdeula/")!

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

    /// One write: its answer into the note, then a re-read.
    ///
    /// The note is cleared first, because the last write's sentence beside this one's outcome
    /// reads as one statement about the wrong thing.
    private func perform(
        _ write: @escaping () async -> AllodiaWriteResult,
        currency: String
    ) {
        guard !writing else { return }
        Task {
            writing = true
            note = nil
            let result = await write()
            writing = false
            note = allodiaWriteNote(result, currency: currency)
            state = await model.allodiaSubscriptionState()
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
