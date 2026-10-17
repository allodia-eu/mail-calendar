// What a new-mail pass says on iPhone and iPad (docs/background-sync.md): the projection's iOS
// branch, which `swift test` never compiles, so this runs only on a simulator
// (clients/apple/Scripts/test-kit.sh --ios). The macOS words are `NewMailNoticesTests`.

#if os(iOS)
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct NewMailNoticesIOSTests {

    private func message(
        subject: String = "Quarterly report",
        preview: String = "The numbers you asked for.",
        key: String = "m1"
    ) -> NewMailPreview {
        NewMailPreview(
            sender: "jane@example.test", senderName: "Jane", subject: subject,
            preview: preview, received: "", messageKey: key)
    }

    private func pass(_ accounts: [AccountNewMail]) -> [NewMailNotice] {
        NewMailNotices.forPass(BackgroundSyncOutcome(accounts: accounts, timedOut: false))
    }

    private func account(
        id: String = "account", newCount: UInt32 = 1, messages: [NewMailPreview]
    ) -> AccountNewMail {
        AccountNewMail(
            accountId: id, accountLabel: "\(id)@example.test", newCount: newCount,
            messages: messages)
    }

    @Test func theAccountComesFirstAndTheSubjectAndSnippetShareTheBody() throws {
        // A phone carrying several accounts needs to know which one first; a banner truncates
        // from the end, so the subject goes above the snippet.
        let notices = pass([account(messages: [message()])])
        try #require(notices.count == 1)
        #expect(notices[0].title == "Jane")
        #expect(notices[0].subtitle == "account@example.test")
        #expect(notices[0].body == "Quarterly report\nThe numbers you asked for.")
    }

    @Test func aMessageWithNoSnippetYetEndsOnItsSubject() throws {
        // An IMAP account has no snippet until the body sync has run, and a notification never
        // ends on a blank line.
        let notices = pass([account(messages: [message(preview: "  \n")])])
        try #require(notices.count == 1)
        #expect(notices[0].body == "Quarterly report")
    }

    @Test func everyNotificationBadgesThePassTotal() {
        // The badge is app-wide, so one account's count must not clobber another's.
        let notices = pass([
            account(id: "a", newCount: 2, messages: [message(key: "m1"), message(key: "m2")]),
            account(id: "b", messages: [message(key: "m3")]),
        ])
        #expect(notices.map(\.badge) == [3, 3, 3])
    }

    @Test func messagesBeyondTheCapEarnASummaryThatQuotesNone() throws {
        let notices = pass([account(newCount: 4, messages: [message()])])
        try #require(notices.count == 2)
        let summary = notices[1]
        #expect(summary.title == L10n.notification_more_messages(count: 3))
        #expect(summary.subtitle == "account@example.test")
        #expect(summary.body == "")
        #expect(summary.badge == 4)
        #expect(summary.target == nil)
    }
}
#endif
