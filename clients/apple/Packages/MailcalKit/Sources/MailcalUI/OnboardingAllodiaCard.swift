// First run: the Allodia-account recommendation, above the address field.
//
// docs/onboarding.md is the contract and decides the order, the card, the way back for someone who
// already has one, a divider naming what follows, then the address field. Its Android, Windows and
// Linux twins draw the same four things in the same order.
//
// Three rules it is easy to break silently:
//
//   * A build with no Allodia registration loses the card, the sign-in line AND the divider
//     together. A lone "or connect directly" heading under nothing is the tell that the wrong thing
//     was gated.
//   * The copy may not out-run the capability matrix: phone and computer, never web.
//   * The card claims the account LIST and nothing else, never the mail, never a password.
//   * Nobody may be stranded on it. This is the one screen a person cannot skip, so the busy state
//     a sign-in puts it in has a way out once the core says it has run long enough to need one.

import MailcalBindings
import SwiftUI

/// The card, the sign-in line and the divider, or nothing at all.
///
/// Once somebody has signed in, the card is replaced by what their other devices hold: the whole
/// reason to sign in here is that the screen that follows is not an empty address field.
struct OnboardingAllodiaCard: View {
    var model: MailboxModel
    /// Sets an offered account up on the route its record names. The whole record, not the
    /// address: the route comes from what the other device wrote down, which is the point of
    /// having synced it.
    let setUp: (AllodiaAccountOffer) -> Void
    /// Whether this is the screen somebody cannot skip. The card is a pitch and is asked once; the
    /// offers are not a pitch, and are shown on any add while they remain.
    var firstRun = true

    var body: some View {
        if allodiaSignInAvailable() {
            if firstRun, let card = model.allodia.card {
                VStack(alignment: .leading, spacing: 12) {
                    content(card)
                    if let failure = card.failure {
                        Text(AllodiaCardRules.failure(failure))
                            .font(.caption)
                            .foregroundStyle(.red)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    divider
                }
            } else if !firstRun, let found, !found.isEmpty {
                // A later add is not pitched the card again, and is still shown the accounts it
                // has left to set up: somebody who set up one of three has two to go.
                VStack(alignment: .leading, spacing: 12) {
                    offerRows(found)
                    divider
                }
            }
        }
    }

    /// What the address field below is, named. Only ever under something: a lone "or connect
    /// directly" heading over nothing is the tell that a client gated the wrong half.
    private var divider: some View {
        VStack(alignment: .leading, spacing: 12) {
            Divider()
            Text(L10n.setup_allodia_divider())
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private func content(_ card: AllodiaCard) -> some View {
        switch card.onboarding {
        case .signingIn:
            busy(L10n.settings_allodia_signing_in()) {
                // Nothing is reported for the way back, and the card returns to the offer it
                // started from.
                if let browser = AllodiaCardRules.escape(card) {
                    ControlButton(title: L10n.action_cancel(), control: browser.cancel) {
                        model.cancelAllodiaSignIn(browser)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.tint)
                }
            }
        // The pass after the sign-in is a bounded network call, not a wait on somebody in
        // another application, and draws no way back.
        case .checking: busy(L10n.settings_allodia_sync_checking()) {}
        // Signed in and asked. Offers become the fast route; none means this account has no mail
        // accounts on it yet.
        case .signedIn: offers
        case .offering: recommendation(card)
        }
    }

    private func busy<Escape: View>(
        _ text: String, @ViewBuilder escape: () -> Escape
    ) -> some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(text).foregroundStyle(.secondary)
            escape()
        }
    }

    /// What the last pass offered, empty when the core says it found nothing, and `nil` otherwise.
    ///
    /// The distinction is the whole of the empty state's correctness: an empty array is "this
    /// account has no mail accounts", `nil` is "we have not looked" or "there is something else to
    /// say", and only the first may say so. Its own property rather than a `let` inside the builder below, so nothing about it
    /// depends on what a result builder accepts.
    private var found: [AllodiaAccountOffer]? {
        guard let devices = model.allodiaDevices else { return nil }
        if devices.nothingFound { return [] }
        return devices.offers.isEmpty ? nil : devices.offers
    }

    /// What a signed-in person is offered, which for a first device is a sentence rather than rows.
    ///
    /// The empty answer is the one worth drawing carefully. Nothing came back, the card is gone,
    /// and what is left under the divider is an address field the person has no reason to connect
    /// with the sign-in they just finished, it reads as the sign-in having failed. So the empty
    /// case says what happened and what to do (`docs/onboarding.md`).
    @ViewBuilder
    private var offers: some View {
        if let found {
            offered(found)
        }
    }

    /// A pass that answered. Empty is a statement; anything else is the rows.
    @ViewBuilder
    private func offered(_ found: [AllodiaAccountOffer]) -> some View {
        if found.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.setup_allodia_none_title()).font(.headline)
                Text(L10n.setup_allodia_none_body())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            offerRows(found)
        }
    }

    /// The accounts the person's other devices hold, as rows.
    @ViewBuilder
    private func offerRows(_ found: [AllodiaAccountOffer]) -> some View {
        Text(L10n.settings_allodia_sync_heading()).font(.headline)
        ForEach(found, id: \.id) { offer in
            HStack {
                Text(offer.email)
                Spacer()
                Button(L10n.settings_allodia_sync_set_up()) { setUp(offer) }
            }
        }
    }

    /// One control rather than a heading beside a button, so a screen reader announces the offer
    /// and its action together, and the label carries the **action**, never the "Recommended"
    /// marker.
    @ViewBuilder
    private func recommendation(_ card: AllodiaCard) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.setup_allodia_recommended())
                    .font(.caption)
                    .foregroundStyle(.tint)
                Text(L10n.setup_allodia_title()).font(.headline)
                Text(L10n.setup_allodia_subtitle())
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                ControlButton(title: L10n.setup_allodia_create(), control: card.create) {
                    model.signInToAllodia(create: true)
                }
                .buttonStyle(.borderedProminent)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(L10n.setup_allodia_title())
        .accessibilityHint(L10n.setup_allodia_subtitle())
        // One line, not a second card of equal weight. `.plain` rather than `.link`, which does
        // not exist on iOS: the weight is what the contract asks for, and a borderless button
        // carries it on both platforms.
        ControlButton(title: L10n.setup_allodia_have_one(), control: card.signIn) {
            model.signInToAllodia()
        }
        .buttonStyle(.plain)
        .foregroundStyle(.tint)
    }
}
