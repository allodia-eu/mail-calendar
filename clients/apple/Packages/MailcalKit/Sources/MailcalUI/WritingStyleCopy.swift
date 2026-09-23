// The words and numbers the Writing style screens, the own AI endpoint panel and the composer's
// Draft a reply control put on screen (docs/ai.md). Plain functions over the core's values, so the
// suite pins which catalog key each outcome reads without drawing a view: the core decides, and a
// failure is a variant a client words, never a server's sentence.

import Foundation
import MailcalBindings

/// What a failed learning run or draft says. `route` decides the one variant whose remedy depends
/// on where requests go: a refused sign-in to the relay and a refused key are different repairs.
func writingStyleFailureText(_ failure: WritingStyleFailure, route: AiRoute?) -> String {
    switch failure {
    case .Unavailable: L10n.ai_error_unavailable()
    case .Busy: L10n.ai_error_busy()
    case .NoSentFolder: L10n.ai_error_no_sent_folder()
    case .NothingToLearn: L10n.learn_report_nothing()
    case .NoStyle: L10n.ai_error_no_style()
    case .NotFound: L10n.ai_error_not_found()
    case let .Refused(mode, _): writingStyleRefusalText(mode)
    case .OutOfCredits: L10n.ai_error_out_of_credits()
    case .NotEntitled: L10n.ai_error_not_entitled()
    case .Unauthorized:
        route == .relay ? L10n.ai_error_sign_in_again() : L10n.ai_error_key_refused()
    case .RateLimited: L10n.ai_error_rate_limited()
    case .Unreachable: L10n.ai_error_unreachable()
    case let .Status(code): L10n.ai_error_status(code: String(code))
    case .Malformed: L10n.ai_error_malformed()
    case .Cancelled: L10n.ai_error_cancelled()
    }
}

/// What the gate's refusal says, by the mode in force. The gate refuses nothing under `.all`, so
/// only the two EU modes have a sentence of their own.
func writingStyleRefusalText(_ mode: JurisdictionMode) -> String {
    mode == .euNative
        ? L10n.writing_style_refused_eu_native()
        : L10n.writing_style_refused_eu_hosted()
}

/// Why the own endpoint was not saved.
func ownEndpointErrorText(_ error: OwnEndpointError) -> String {
    switch error {
    case .InvalidUrl: L10n.ai_endpoint_error_address()
    case .NotHttps: L10n.ai_endpoint_error_https()
    case .NoModel: L10n.ai_endpoint_error_model()
    case .Keystore: L10n.ai_endpoint_error_keystore()
    }
}

/// The key `set_own_ai_endpoint` is handed: `nil` keeps the stored one, so an empty field never
/// erases a key the screen cannot show.
func ownEndpointKeyArgument(_ field: String) -> String? {
    let key = field.trimmingCharacters(in: .whitespacesAndNewlines)
    return key.isEmpty ? nil : key
}

/// Whether a composer offers Draft a reply: a reply or a reply all, while AI has somewhere to go.
func offersDraftReply(mode: RichComposeMode, route: AiRoute?) -> Bool {
    route != nil && (mode == .reply || mode == .replyAll)
}

/// The `from` a draft is asked for: the sending account only when the person moved the reply off
/// the account that received the message, since the core already drafts from that one.
func draftReplyFrom(answered: String, sending: String?) -> String? {
    guard let sending, sending != answered else { return nil }
    return sending
}

/// The intent field's text, or `nil` when it holds nothing but space.
func draftReplyIntent(_ text: String) -> String? {
    let intent = text.trimmingCharacters(in: .whitespacesAndNewlines)
    return intent.isEmpty ? nil : intent
}

/// The credits line: at most one decimal in the locale's digits, and when the relay said so, as
/// the list row dates a message (the clock today, a weekday this week, the date before that).
func writingStyleCreditsLine(
    _ balance: CreditBalance, now: Date, zone: TimeZone, locale: Locale
) -> String {
    let number = NumberFormatter()
    number.locale = locale
    number.numberStyle = .decimal
    number.minimumFractionDigits = 0
    number.maximumFractionDigits = 1
    let credits = number.string(from: NSNumber(value: balance.credits)) ?? "\(balance.credits)"

    let asOf = Date(timeIntervalSince1970: TimeInterval(balance.asOf))
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = zone
    let dayDiff = calendar.dateComponents(
        [.day], from: calendar.startOfDay(for: asOf), to: calendar.startOfDay(for: now)
    ).day ?? 0
    let sameYear = calendar.component(.year, from: asOf) == calendar.component(.year, from: now)
    let time = DateFormatter()
    time.locale = locale
    time.timeZone = zone
    time.dateFormat = relativeDatePattern(dayDiff: dayDiff, sameYear: sameYear)
    return L10n.writing_style_credits(credits: credits, time: time.string(from: asOf))
}

/// A date the Writing style screens state (when a style was learned, how far back the device
/// holds sent mail): the day alone, in the display zone.
func writingStyleDate(_ seconds: Int64, zone: TimeZone, locale: Locale) -> String {
    let formatter = DateFormatter()
    formatter.locale = locale
    formatter.timeZone = zone
    formatter.dateStyle = .medium
    formatter.timeStyle = .none
    return formatter.string(from: Date(timeIntervalSince1970: TimeInterval(seconds)))
}

/// Languages by their own names ("Nederlands", "Deutsch"), as a list in the app's language.
func writingStyleLanguages(_ codes: [String], locale: Locale) -> String {
    codes.map(L10n.languageName).formatted(.list(type: .and).locale(locale))
}

/// A habit and how often it is used: "Hi Bob, (80%)", the percentage in the locale's form.
func writingStyleHabit(_ habit: HabitRow, locale: Locale) -> String {
    let share = (Double(habit.share) / 100).formatted(.percent.locale(locale))
    return "\(habit.text) (\(share))"
}
