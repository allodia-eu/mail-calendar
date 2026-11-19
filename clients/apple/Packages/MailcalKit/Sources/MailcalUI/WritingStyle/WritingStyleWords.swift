// The catalog's words for Settings → Writing style, its learning flow, the reveal and the detail
// (docs/ai.md rules 8 to 16). The core decides what each record says; this only words it and
// formats its days and dates for the reader. Plain functions, so the unit suite reads them without
// a screen. A failure and its remedy are `AiWords`'s, shared with Draft a reply.

import Foundation
import MailcalBindings

enum WritingStyleWords {
    /// A `YYYY-MM-DD` day from the core as a date the reader knows, or the text itself when it is
    /// not one. A day is a whole day in UTC, so it is read and shown in UTC.
    static func day(_ text: String, locale: Locale = L10n.appLocale) -> String {
        guard let date = dayDate(text) else { return text }
        return date.formatted(
            Date.FormatStyle(date: .long, time: .omitted, locale: locale, timeZone: utc))
    }

    /// A `YYYY-MM-DD` day as the start of that day in UTC, for a date picker.
    static func dayDate(_ text: String) -> Date? {
        let parts = text.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3 else { return nil }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = utc
        return calendar.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))
    }

    /// A date picked as the `YYYY-MM-DD` day the core reads, in UTC.
    static func dayText(_ date: Date) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = utc
        let parts = calendar.dateComponents([.year, .month, .day], from: date)
        return String(format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0)
    }

    /// Seconds since the epoch as a date in the reader's own zone.
    static func date(
        _ seconds: Int64, zone: TimeZone = .current, locale: Locale = L10n.appLocale
    ) -> String {
        Date(timeIntervalSince1970: TimeInterval(seconds)).formatted(
            Date.FormatStyle(date: .abbreviated, time: .omitted, locale: locale, timeZone: zone))
    }

    /// Language codes as their names in the reader's language, joined as a list.
    static func languages(_ codes: [String], locale: Locale = L10n.appLocale) -> String {
        codes.map { language($0, locale: locale) }.formatted(.list(type: .and).locale(locale))
    }

    /// One language code's name in the reader's language.
    static func language(_ code: String, locale: Locale = L10n.appLocale) -> String {
        locale.localizedString(forLanguageCode: code)?.capitalized(with: locale) ?? code
    }

    /// The line under a library row.
    static func facts(_ facts: WritingStyleFacts, zone: TimeZone = .current) -> String {
        let count = Int(facts.messages)
        guard let learned = facts.learnedAt else {
            return facts.learnedFrom
        }
        return L10n.writing_style_learned_from(count: count, date: date(learned, zone: zone))
    }

    /// A preset range's label.
    static func range(_ range: StyleLearnRange) -> String {
        switch range {
        case .everything: L10n.learn_range_all()
        case .untilEndOfLastYear: L10n.learn_range_until_last_year()
        case .between: L10n.learn_range_between()
        }
    }

    /// The range read, as the review names it: a preset by its name, a range of the person's own
    /// by its two days.
    static func rangeRead(_ range: StyleLearnRange, locale: Locale = L10n.appLocale) -> String {
        guard case let .between(since, until) = range,
              let from = dayDate(since), let to = dayDate(until), from <= to
        else { return Self.range(range) }
        var style = Date.IntervalFormatStyle(date: .long, time: .omitted, locale: locale)
        style.timeZone = utc
        return (from..<to.addingTimeInterval(1)).formatted(style)
    }

    /// How far back the device holds the account's sent mail.
    static func reach(_ reach: StyleSentReach) -> String {
        switch reach {
        case .everything: L10n.learn_report_horizon_all()
        case let .since(day): L10n.learn_report_horizon(date: Self.day(day))
        }
    }

    /// The run's progress: a fraction once the run knows its total.
    static func progress(done: UInt32, total: UInt32) -> String {
        total == 0
            ? L10n.learn_progress_starting()
            : L10n.learn_progress(done: String(done), total: String(total))
    }

    /// What is sent where, naming the relay or the endpoint's host.
    static func consent(_ destination: StyleLearningDestination) -> String {
        switch destination {
        case .relay: L10n.learn_consent_relay()
        case let .ownEndpoint(host): L10n.learn_consent_own(host: host)
        }
    }

    /// Why Learn cannot be pressed.
    static func blocked(_ block: WritingStyleLearnBlock) -> String {
        switch block {
        case .noMailAccount: L10n.writing_style_learn_no_account()
        case let .cannotSend(failure, _): AiWords.failure(failure)
        }
    }

    /// Why a change to the library was not made.
    static func refusal(_ refusal: WritingStyleRefusal) -> String {
        switch refusal {
        case .blankName: L10n.writing_style_refused_blank_name()
        case .gone: L10n.writing_style_refused_gone()
        }
    }

    /// How often a greeting or sign-off is used.
    static func frequency(_ frequency: WritingStyleFrequency) -> String {
        switch frequency {
        case .mostly: L10n.reveal_frequency_mostly()
        case .often: L10n.reveal_frequency_often()
        case .sometimes: L10n.reveal_frequency_sometimes()
        }
    }

    /// What a card of tone and approach is about.
    static func topic(_ topic: WritingStyleTopic) -> String {
        switch topic {
        case .register: L10n.reveal_register()
        case .structure: L10n.reveal_structure()
        case .moves: L10n.reveal_moves()
        case .punctuation: L10n.reveal_punctuation()
        }
    }

    /// A reveal page's title.
    static func page(_ page: WritingStyleRevealPage) -> String {
        switch page {
        case .read: L10n.reveal_title()
        case .letter: L10n.reveal_step_letter()
        case .habits: L10n.reveal_step_habits()
        case .voice: L10n.reveal_step_voice()
        case .phrases: L10n.reveal_phrases()
        case .name: L10n.reveal_step_name()
        }
    }

    /// The question Forget asks: what is removed, which accounts draft in it, and that no mail
    /// is touched.
    static func forgetMessage(_ accounts: [String], locale: Locale = L10n.appLocale) -> String {
        let message = L10n.writing_style_forget_message()
        guard !accounts.isEmpty else { return message }
        let listed = accounts.formatted(.list(type: .and).locale(locale))
        return L10n.writing_style_forget_accounts(accounts: listed) + "\n\n" + message
    }

    private static let utc = TimeZone(identifier: "UTC") ?? .gmt
}
