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
//   * The copy may not out-run the README capability matrix: phone and computer, never web.
//   * The card claims the account LIST and nothing else, never the mail, never a password.
//   * Nobody may be stranded on it. This is the one screen a person cannot skip, so the busy state
//     a sign-in puts it in always has a way out.

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

    /// Who is signed in. Read on appear and after every attempt; the answer is local and never asks
    /// the service.
    @State private var account: AllodiaAccount?
    @State private var signingIn = false
    @State private var failure: String?
    /// The running sign-in, held so it can be retired. Cancelling it is what stops an attempt
    /// somebody escaped from opening a browser or storing a grant behind their back.
    @State private var signIn: Task<Void, Never>?
    /// Whether the hop has outlasted its threshold, so the busy row draws the way back.
    @State private var escapable = false

    /// How long a hop runs before the card owes the person a way back. The contract caps this at
    /// ten seconds; eight matches Android and Linux. An ordinary hop has the browser in front of
    /// somebody inside a second, and a button drawn for that one is noise on every sign-in that
    /// works.
    static let escapeAfter = Duration.seconds(8)

    var body: some View {
        if allodiaSignInAvailable() {
            if firstRun {
                VStack(alignment: .leading, spacing: 12) {
                    content
                    if let failure {
                        Text(L10n.settings_allodia_failed(error: failure))
                            .font(.caption)
                            .foregroundStyle(.red)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    divider
                }
                // Both belong to the card: only it can be mid-sign-in, and only it reads who is
                // signed in. A later add shows rows and needs neither.
                .task { account = model.currentAllodiaAccount() }
                .task(id: signingIn) { await armEscape() }
            } else if let found, !found.isEmpty {
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
    private var content: some View {
        if signingIn || (account != nil && model.allodiaSync.checking) {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(
                    signingIn
                        ? L10n.settings_allodia_signing_in()
                        : L10n.settings_allodia_sync_checking()
                )
                .foregroundStyle(.secondary)
                // Only for the browser leg: the pass below it is a bounded network call, not a
                // wait on somebody in another application, and draws no way back.
                if signingIn, escapable {
                    Button(L10n.action_cancel()) { escape() }
                        .buttonStyle(.plain)
                        .foregroundStyle(.tint)
                }
            }
        } else if account != nil {
            // Signed in and asked. Offers become the fast route; none means this account has no
            // mail accounts on it yet.
            offers
        } else {
            recommendation
        }
    }

    /// What the last pass answered with, or `nil` while none has answered.
    ///
    /// The distinction is the whole of the empty state's correctness: an empty array is "this
    /// account has no mail accounts", `nil` is "we have not looked", and only the first may say
    /// so. Its own property rather than a `let` inside the builder below, so nothing about it
    /// depends on what a result builder accepts.
    private var found: [AllodiaAccountOffer]? {
        model.allodiaSync.report?.offers
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
    private var recommendation: some View {
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
                Button(L10n.setup_allodia_create()) { begin(create: true) }
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
        Button(L10n.setup_allodia_have_one()) { begin(create: false) }
            .buttonStyle(.plain)
            .foregroundStyle(.tint)
    }

    /// Arms the way back rather than drawing it: a hop that comes straight back never puts a button
    /// in front of somebody who had no reason to read it.
    ///
    /// Owned by the busy state through `.task(id: signingIn)`, so SwiftUI restarts this clock when
    /// a hop begins and cancels it when one ends, an attempt can never be armed by an older one's
    /// timer, and there is no timer left running for a card that has gone.
    private func armEscape() async {
        escapable = false
        guard signingIn else { return }
        try? await Task.sleep(for: Self.escapeAfter)
        // `try?` swallows the cancellation that ending the hop raises, so the check is here.
        if !Task.isCancelled { escapable = true }
    }

    private func begin(create: Bool) {
        failure = nil
        signingIn = true
        signIn = Task {
            let outcome = await model.signInToAllodia(create: create)
            // An attempt the person escaped: they are back at the offer already, and nothing this
            // one answers belongs on a screen that has moved on.
            guard !Task.isCancelled else { return }
            signingIn = false
            switch outcome {
            case let .signedIn(signedIn):
                account = signedIn
                // What their other devices hold, before they are asked to type anything.
                await model.syncAllodiaAccounts()
            // A dismissed browser is not a failure, say nothing and leave the card as it was.
            case .cancelled: break
            case let .failed(error): failure = error
            }
        }
    }

    /// The person's way out of a hop that did not come back. Nothing is reported: a sign-in
    /// somebody abandoned is not a failure, and the card returns to the offer it started from.
    private func escape() {
        signIn?.cancel()
        signIn = nil
        signingIn = false
    }
}
