// Which of the core's notices a screen draws, and what an account carries (docs/notices.md). The
// core decides and ranks every notice; this only filters, so the order stays the core's and
// every platform shows the same notice first.

import MailcalBindings

enum NoticeRules {
    /// Whether `notice` belongs on `destination`.
    static func belongs(_ notice: Notice, on destination: AppDestination) -> Bool {
        switch notice.screens {
        case .everywhere: true
        case .mail: destination == .mail
        case .calendar: destination == .calendar
        case .contacts: destination == .contacts
        }
    }

    /// The notices `destination` shows, in the core's order: the first is drawn, the rest sit
    /// behind one Issues entry (rule 2).
    static func shown(_ notices: [Notice], on destination: AppDestination) -> [Notice] {
        notices.filter { belongs($0, on: destination) }
    }

    /// Every notice about `account`, whatever its screen, in the core's order: the first marks
    /// the account's row, and all of them head its Settings page (rule 6).
    static func about(_ account: String, in notices: [Notice]) -> [Notice] {
        notices.filter { $0.account == account }
    }

    /// What an account's row says under its address: the first notice's few words, or how many
    /// there are when there are several (rule 6). `nil` when the account has none.
    static func rowLabel(_ about: [Notice]) -> String? {
        if about.count > 1 { return L10n.notices_account_count(count: about.count) }
        return about.first?.mark.map(text)
    }

    /// Whether the account service refused the Allodia sign-in, which the Allodia card draws.
    static func allodiaSignInEnded(_ notices: [Notice]) -> Bool {
        notices.contains { notice in
            if case .allodiaSignInEnded = notice.kind { return true }
            return false
        }
    }

    /// The notices in `current` that were not in force before: the ones to announce (rule 12).
    static func raised(_ current: [Notice], since previous: [Notice]) -> [Notice] {
        let before = Set(previous.map(\.id))
        return current.filter { !before.contains($0.id) }
    }

    /// The words, through the lookup generated from the catalog (rule 11).
    static func text(_ text: NoticeText) -> String {
        L10n.notice(text.key, text.args)
    }

    /// What is said under the title: that the remedy did not work, else why, if the core says.
    static func detail(_ notice: Notice) -> String? {
        if notice.remedyFailed { return L10n.notices_remedy_failed() }
        return notice.detail.map(text)
    }

    /// The glyph, its shape differing by severity so colour is never the only signal (rule 4).
    static func symbol(_ severity: NoticeSeverity) -> String {
        switch severity {
        case .blocked: "exclamationmark.triangle.fill"
        case .degraded: "exclamationmark.circle.fill"
        case .informational: "info.circle.fill"
        }
    }

    /// Only Blocked takes the warning colour (rule 4).
    static func isWarning(_ severity: NoticeSeverity) -> Bool {
        severity == .blocked
    }
}
