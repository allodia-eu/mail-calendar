// Which of the core's notices each screen draws, what an account carries, what is announced, and
// the words (docs/notices.md). The core ranks; these hold the client to its order.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct NoticeRulesTests {
    private func notice(
        _ id: String,
        _ screens: NoticeScreens,
        account: String? = nil,
        severity: NoticeSeverity = .degraded,
        failed: Bool = false,
        detail: NoticeText? = nil,
        kind: NoticeKind = .offline
    ) -> Notice {
        Notice(
            id: id,
            kind: kind,
            title: NoticeText(key: "notice_offline", args: [:]),
            detail: detail,
            severity: severity,
            screens: screens,
            account: account,
            mark: account.map { _ in NoticeText(key: "notice_mark_signin_expired", args: [:]) },
            action: nil,
            actionLabel: nil,
            dismissible: false,
            remedyFailed: failed
        )
    }

    @Test func aScreenShowsItsOwnAndEverywhereNoticesInTheCoresOrder() {
        let ranked = [
            notice("calendar", .calendar),
            notice("mail", .mail),
            notice("everywhere", .everywhere),
            notice("contacts", .contacts),
        ]
        #expect(NoticeRules.shown(ranked, on: .mail).map(\.id) == ["mail", "everywhere"])
        #expect(NoticeRules.shown(ranked, on: .calendar).map(\.id) == ["calendar", "everywhere"])
        #expect(NoticeRules.shown(ranked, on: .contacts).map(\.id) == ["everywhere", "contacts"])
    }

    @Test func anAccountCarriesEveryNoticeAboutItWhateverItsScreen() {
        let ranked = [
            notice("a-calendar", .calendar, account: "a"),
            notice("b-mail", .mail, account: "b"),
            notice("app", .everywhere),
            notice("a-mail", .mail, account: "a"),
        ]
        #expect(NoticeRules.about("a", in: ranked).map(\.id) == ["a-calendar", "a-mail"])
        #expect(NoticeRules.about("c", in: ranked).isEmpty)
    }

    @Test func anAccountRowSaysTheFewWordsOrHowManyThereAre() {
        let one = [notice("a-mail", .mail, account: "a")]
        #expect(NoticeRules.rowLabel(one) == L10n.notice_mark_signin_expired())
        let two = one + [notice("a-calendar", .calendar, account: "a")]
        #expect(NoticeRules.rowLabel(two) == L10n.notices_account_count(count: 2))
        #expect(NoticeRules.rowLabel([]) == nil)
    }

    @Test func onlyANoticeNotInForceBeforeIsAnnounced() {
        let before = [notice("a", .mail), notice("b", .mail)]
        let after = [notice("b", .mail), notice("c", .mail)]
        #expect(NoticeRules.raised(after, since: before).map(\.id) == ["c"])
        #expect(NoticeRules.raised(after, since: after).isEmpty, "a redraw announces nothing")
    }

    @Test func theWordsComeFromTheGeneratedLookup() {
        let title = NoticeText(key: "notice_signin_expired", args: ["address": "anna@example.org"])
        #expect(NoticeRules.text(title) == L10n.notice_signin_expired(address: "anna@example.org"))
    }

    @Test func aFailedRemedyIsSaidInPlaceOfTheDetail() {
        let why = NoticeText(key: "notice_detail_unreachable_calendar", args: [:])
        #expect(NoticeRules.detail(notice("a", .calendar, detail: why))
            == L10n.notice_detail_unreachable_calendar())
        #expect(NoticeRules.detail(notice("a", .calendar, failed: true, detail: why))
            == L10n.notices_remedy_failed())
        #expect(NoticeRules.detail(notice("a", .calendar)) == nil)
    }

    @Test func eachSeverityHasItsOwnShapeAndOnlyBlockedTheWarningColour() {
        let severities: [NoticeSeverity] = [.blocked, .degraded, .informational]
        #expect(Set(severities.map(NoticeRules.symbol)).count == 3)
        #expect(severities.filter(NoticeRules.isWarning) == [.blocked])
    }
}
