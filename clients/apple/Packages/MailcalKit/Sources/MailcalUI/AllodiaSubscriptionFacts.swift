// How the subscription section words what the core decided (`allodiaSubscriptionCard`): who is
// charging, what a buy button says, what a write came to, a date and an amount in minor units.
// Pure and SwiftUI-free, so each sentence is asserted in a unit test. Linux's twin is
// `allodia_subscription_facts.rs`; keep the wording in step with every client's.

import Foundation
import MailcalBindings

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

extension AllodiaStore {
    /// What a store is called on its page's button.
    var displayName: String {
        switch self {
        case .apple: return "Apple"
        case .google: return "Google Play"
        // The service never reports its own billing as a store.
        case .allodia: return "Allodia"
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

    /// The button that moves Allodia's own subscription to this period.
    var switchLabel: String {
        switch self {
        case .yearly: return L10n.settings_subscription_switch_yearly()
        case .monthly: return L10n.settings_subscription_switch_monthly()
        }
    }
}

/// Whether the App Store's offers are fetched and drawn: only on an answer that grants nothing,
/// since each fetch is a StoreKit round trip and the service refuses a second subscription.
func allodiaSellsHere(_ state: AllodiaSubscriptionState?) -> Bool {
    if case let .loaded(view)? = state { return !view.entitled }
    return false
}

/// The sentence what a write came to earns.
@MainActor func allodiaOutcomeText(_ outcome: AllodiaSubscriptionOutcome) -> String {
    switch outcome {
    case let .cancelled(endDate):
        return L10n.settings_subscription_cancelled(date: allodiaDate(endDate) ?? endDate)
    case .restarted:
        return L10n.settings_subscription_resubscribed()
    case let .switched(nextPaymentDate, amountInCents, currency):
        return L10n.settings_subscription_switched(
            date: nextPaymentDate.flatMap(allodiaDate) ?? "",
            amount: allodiaMinorUnits(amountInCents, currency: currency)
        )
    case let .failed(failure):
        return allodiaRefusalText(failure)
    }
}

/// A refusal's own words.
///
/// ⚠️ **A code, never a message from anywhere else**: "you have already cancelled" and "that
/// cannot change while a charge is being retried" are different things to say, and only the code
/// tells them apart.
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

/// Minor units and a currency as something a reader recognises.
///
/// ⚠️ **Only for what Allodia bills directly.** A store's price is the store's own formatted
/// string, drawn and never recomputed.
@MainActor func allodiaMinorUnits(_ minorUnits: Int64, currency: String?) -> String {
    let amount = Decimal(minorUnits) / 100
    guard let currency, !currency.isEmpty else { return "\(amount)" }
    allodiaAmountFormatter.currencyCode = currency
    return allodiaAmountFormatter.string(from: amount as NSDecimalNumber) ?? "\(amount)"
}

/// Built once: `NumberFormatter` loads ICU locale data on construction, which the date helpers
/// below carry the same warning about.
@MainActor private let allodiaAmountFormatter: NumberFormatter = {
    let output = NumberFormatter()
    output.numberStyle = .currency
    return output
}()

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
/// ⚠️ **`parseUtcInstant` alone is not enough.** It requires a `Z` suffix because the engine's
/// timestamps carry one; the account service is a different server and sends a numeric offset,
/// which it would leave as the raw string, drawn as `2026-09-18` to a Dutch reader who should see
/// `18 september 2026`.
private func allodiaInstant(_ raw: String) -> Date? {
    if let date = parseUtcInstant(raw) { return date }
    for parser in allodiaIsoParsers {
        if let date = parser.date(from: raw) { return date }
    }
    return allodiaDayParser.date(from: String(raw.prefix(10)))
}

/// Built once each: `ISO8601DateFormatter` loads ICU data on construction, the same cost the
/// timestamp helpers carry a warning about. Fractional seconds are a separate parser rather than a
/// flag, because the option makes a formatter **require** them rather than allow them, so one
/// configuration cannot read both shapes.
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
/// carry the same warning about.
@MainActor private let allodiaDateFormatter: DateFormatter = {
    let output = DateFormatter()
    output.dateStyle = .long
    output.timeStyle = .none
    return output
}()
