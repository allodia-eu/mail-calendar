// Settings → Allodia account → Subscription: what is being charged, by whom, until when, and the
// way to start one. Its Android, Windows and Linux twins say the same things in the same order;
// keep the wording in step.
//
// **What this file draws is what the core decided** (`allodiaSubscriptionCard`, `purchasing.md`):
// who is billing, which store pages to offer, which writes exist, whether one runs and what it
// came to. A store's subscription is changed only at that store, so its button opens that store's
// page. Buying is the App Store's, so the offers, the purchase sheet and the redemption pass stay
// this client's.
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

/// Settings → Allodia account → Subscription, drawn while the core draws one: somebody is signed
/// in and the service still accepts the sign-in.
struct AllodiaSubscriptionSettings: View {
    var model: MailboxModel

    @Environment(\.openURL) private var openURL
    @Environment(\.scenePhase) private var scenePhase
    /// What the App Store sells, in the core's order. Fetched only while the account is free: it
    /// costs a StoreKit round trip.
    @State private var offers: [AllodiaOffer] = []
    /// Which period has a purchase sheet up, so its button alone shows the spinner.
    @State private var buying: AllodiaPlan?
    /// The wait on that sheet, held so leaving the screen can let go of it.
    ///
    /// ⚠️ **StoreKit puts no bound on `purchase()`, and a sheet that never answers suspends its
    /// caller for the life of the process.** Without this the screen has no way back: every button
    /// stays disabled behind a spinner that will not stop, and only relaunching the app clears it.
    @State private var purchase: Task<Void, Never>?
    /// What the last redemption pass owes a sentence about.
    @State private var pass = AllodiaPassNotes()
    /// The last thing worth saying about a purchase: a pending approval, or a failure in the
    /// store's own words. Cleared when the next attempt starts.
    @State private var note: String?

    private var card: AllodiaSubscriptionCard? { model.allodia.subscription }

    private var sellsHere: Bool { allodiaSellsHere(card?.state) }

    var body: some View {
        Group {
            if let card, card.state != .hidden {
                GroupBox {
                    VStack(alignment: .leading, spacing: 10) {
                        heading(card.state)
                        content(card.state)
                        if let outcome = card.outcome {
                            caption(allodiaOutcomeText(outcome))
                        }
                        if let note { caption(note) }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(6)
                }
            }
        }
        // Read when the section opens, after a redemption pass, so a purchase made on another
        // device or one this device could not attach last time is in the answer.
        .task { pass = await model.linkAllodiaPurchasesThenRefresh() }
        // Back from the App Store's subscription page, or from anywhere else: read again. A read
        // still running answers for this one.
        .onChange(of: scenePhase) { _, phase in
            if phase == .active { model.refreshAllodiaSubscription() }
        }
        .task(id: sellsHere) {
            guard sellsHere else {
                offers = []
                return
            }
            offers = await model.allodiaPurchases?.offers() ?? []
        }
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

    /// The heading, and the spinner of a read running beside an answer that stays drawn.
    private func heading(_ state: AllodiaSubscriptionState) -> some View {
        HStack(spacing: 8) {
            Text(L10n.settings_subscription_heading()).font(.headline)
            if case let .loaded(view) = state, view.refreshing {
                ProgressView().controlSize(.small)
            }
        }
    }

    @ViewBuilder
    private func content(_ state: AllodiaSubscriptionState) -> some View {
        switch state {
        case .hidden:
            // Never drawn: the section hides itself in this state (see `body`).
            EmptyView()
        case .checking:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L10n.settings_subscription_checking()).foregroundStyle(.secondary)
            }
        case .unavailable:
            // Deliberately quiet. A read that did not arrive costs a sentence, never access:
            // whether a capability is on is `allodiaEntitlement`'s answer, and it is local.
            callout(L10n.settings_subscription_unavailable())
        case .needsReauth:
            // An offer rather than an error: they are signed in and this one read is asleep, and
            // the ordinary sign-in asks for the full current scope set.
            callout(L10n.settings_subscription_reauth())
            signInAgain
        case let .loaded(view):
            loaded(view)
        }
    }

    @ViewBuilder
    private func loaded(_ view: AllodiaSubscriptionView) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            if view.entitled {
                active(view)
            } else {
                callout(L10n.settings_subscription_free())
                // ⚠️ **A device that cannot name its account does not buy here.** The id a purchase
                // is tagged with is the only way to attribute one whose report never arrives, and
                // it cannot be added afterwards, so a grant stored before it existed is asked to
                // sign in again rather than allowed to make a purchase that can never be repaired.
                if AllodiaCardRules.account(model.allodia.card?.state)?.id == nil {
                    callout(L10n.settings_subscription_reauth())
                    signInAgain
                } else {
                    offerButtons
                }
                // On hold and paused grant nothing and are put right only at the store.
                storePages(view)
            }
            // ⚠️ Money was taken and nothing has been granted. A screen that stays silent here
            // leaves somebody with no way to find that out.
            if pass.stuck { caption(L10n.settings_subscription_stuck()) }
            // ⚠️ What they bought is on a different Allodia account, so it is never coming to this
            // one and no amount of waiting changes that.
            if pass.claimedElsewhere { caption(L10n.settings_subscription_claimed_elsewhere()) }
        }
    }

    /// What a paid account says: who is charging, until when, and where to change it.
    @ViewBuilder
    private func active(_ view: AllodiaSubscriptionView) -> some View {
        if let biller = view.billedBy {
            Text(L10n.settings_subscription_billed_by(biller: biller.displayName))
                .font(.callout)
        }
        if let end = view.periodEnd.flatMap(allodiaDate) {
            Text(
                view.renews
                    ? L10n.settings_subscription_renews(date: end)
                    : L10n.settings_subscription_ends(date: end)
            )
            .font(.callout)
            .foregroundStyle(.secondary)
        }
        if view.grace, let biller = view.billedBy {
            // Not a lapse, and never drawn as one: the charge is being retried and access
            // continues.
            caption(L10n.settings_subscription_grace(biller: biller.displayName))
        }
        // Reported, never resolved. Cancelling one of them without asking is a decision about
        // somebody else's money, so each exit is offered and none is taken.
        if !view.duplicateBilling.isEmpty {
            Text(
                L10n.settings_subscription_duplicate(
                    billers: view.duplicateBilling.map(\.displayName).joined(separator: ", ")
                )
            )
            .font(.caption)
            .foregroundStyle(.orange)
            .fixedSize(horizontal: false, vertical: true)
        }
        storePages(view)
        AllodiaSubscriptionWriteButtons(view: view, model: model)
    }

    /// One button per store with something left to manage there. Review-blocking on both stores.
    private func storePages(_ view: AllodiaSubscriptionView) -> some View {
        ForEach(view.storePages, id: \.self) { page in
            Button(L10n.settings_subscription_manage(biller: page.store.displayName)) {
                manage(page)
            }
        }
    }

    /// The plans the App Store sells, in the core's order.
    @ViewBuilder
    private var offerButtons: some View {
        ForEach(offers.indices, id: \.self) { index in
            let offer = offers[index]
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
            caption(L10n.settings_subscription_terms())
        }
        // Apple's guideline 3.1.2: a screen selling an auto-renewable subscription links the
        // Terms of Use and the privacy policy, beside the purchase rather than elsewhere in the
        // app. Every purchase here is StoreKit's, so the terms are Apple's standard EULA, the one
        // the App Store listing links too. The Android, Windows and Linux twins draw no such line:
        // Apple's EULA governs nothing Google or Allodia bills.
        if !offers.isEmpty {
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
    /// so there the page the core named is opened instead. A store that is not Apple is a URL on
    /// both, which is what Play's deep link already is.
    private func manage(_ page: AllodiaStorePage) {
        #if os(iOS)
        if page.store == .apple {
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
        if let url = URL(string: page.url) { openURL(url) }
    }

    /// Signs in again, which is what records the account id on a grant stored without one, and
    /// what gives an old grant the permission the read needs. The core reads the section again
    /// once it ends.
    private var signInAgain: some View {
        ControlButton(
            title: L10n.settings_allodia_reauth_action(), control: model.allodiaSignInControl
        ) {
            model.signInToAllodia()
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
                // Read rather than assume: what was bought is granted only once the account
                // service has attached it, and the read is what says whether it has.
                pass = await model.linkAllodiaPurchasesThenRefresh()
            case .awaitingApproval:
                note = L10n.settings_subscription_pending()
            // A dismissed sheet is not a failure. Say nothing and leave the button as it was.
            case .cancelled: break
            case let .failed(error):
                note = L10n.settings_subscription_failed(error: error)
            }
        }
    }

    private func callout(_ text: String) -> some View {
        Text(text)
            .font(.callout)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func caption(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }
}
