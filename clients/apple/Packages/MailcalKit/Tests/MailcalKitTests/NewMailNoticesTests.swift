// What a new-mail pass says on macOS (docs/background-sync.md), the host-testable half of the
// notification: the projection decides the words and the identities, `MailNotifier` only hands
// them to `UNUserNotificationCenter`. The Apple twin of the Linux client's `notification_parts`
// tests and Windows's `NewMailNoticesTests`.
//
// macOS-only because the suite runs on the host: the iOS branch of the projection compiles for a
// different platform and this target never sees it.

#if os(macOS)
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct NewMailNoticesTests {

    private func message(
        sender: String = "jane@example.test",
        senderName: String? = "Jane",
        subject: String = "Quarterly report",
        preview: String = "The numbers you asked for.",
        key: String = "m1"
    ) -> NewMailPreview {
        NewMailPreview(
            sender: sender,
            senderName: senderName,
            subject: subject,
            preview: preview,
            received: "",
            messageKey: key
        )
    }

    private func account(
        id: String = "account",
        label: String = "account@example.test",
        newCount: UInt32 = 1,
        messages: [NewMailPreview]
    ) -> AccountNewMail {
        AccountNewMail(
            accountId: id, accountLabel: label, newCount: newCount, messages: messages)
    }

    private func pass(_ accounts: [AccountNewMail]) -> [NewMailNotice] {
        NewMailNotices.forPass(BackgroundSyncOutcome(accounts: accounts, timedOut: false))
    }

    /// The one notification a single-message pass raises.
    private func only(_ message: NewMailPreview) throws -> NewMailNotice {
        let notices = pass([account(messages: [message])])
        try #require(notices.count == 1)
        return notices[0]
    }

    // MARK: who it is from, what it is about, how it begins

    @Test func oneMessageFillsTheThreeSlotsInOrder() throws {
        let notice = try only(message())
        #expect(notice.title == "Jane")
        #expect(notice.subtitle == "Quarterly report")
        #expect(notice.body == "The numbers you asked for.")
    }

    @Test func theSnippetIsQuotedTheWayTheListRowQuotesIt() throws {
        // A real snippet carries the body's own line breaks, and a bounce report carries blank
        // lines between paragraphs. The row collapses them and so does this, or the two would
        // quote the same message differently.
        let notice = try only(
            message(
                senderName: "Mail Delivery Subsystem",
                subject: "Failed to deliver message",
                preview: "Your message could not be delivered:\n\n<news@example.com>\n\n"
            ))
        #expect(notice.body == "Your message could not be delivered: <news@example.com>")
    }

    @Test func aMessageWithNoSnippetYetIsItsSubjectAlone() throws {
        // An IMAP account has no snippet until the body sync has run. The subject moves down into
        // the body rather than standing alone above an empty one.
        let notice = try only(message(preview: ""))
        #expect(notice.subtitle == "")
        #expect(notice.body == "Quarterly report")
    }

    @Test func aMessageWithNoSubjectIsItsSnippetAlone() throws {
        let notice = try only(message(subject: ""))
        #expect(notice.subtitle == "")
        #expect(notice.body == "The numbers you asked for.")
    }

    // MARK: the title is never blank

    @Test func aSenderWithNoDisplayNameIsItsAddress() throws {
        let notice = try only(message(senderName: nil))
        #expect(notice.title == "jane@example.test")
    }

    @Test func aMessageNamingNoSenderAtAllGetsALineOfItsOwn() throws {
        // The same rule the avatar's monogram follows: a notification is never blank.
        let notice = try only(message(sender: "  ", senderName: "  "))
        #expect(notice.title == L10n.notification_unknown_sender())
    }

    // MARK: one notification per message, and a summary for the rest

    @Test func eachMessageKeepsAnIdentityOfItsOwn() {
        // Keying by message rather than by account is a contract requirement: the high-water mark
        // advances past reported mail, so a per-account identifier would let the next pass replace
        // an earlier still-unseen notification and lose it for good.
        let notices = pass([
            account(
                newCount: 2,
                messages: [message(subject: "One", key: "m1"), message(subject: "Two", key: "m2")]
            )
        ])
        #expect(notices.map(\.identifier) == ["new-mail-m1", "new-mail-m2"])
        #expect(notices.allSatisfy { $0.threadIdentifier == "account" })
    }

    @Test func aPassThatFittedSaysNothingExtra() {
        #expect(pass([account(newCount: 1, messages: [message()])]).count == 1)
    }

    @Test func onlyTheMessagesBeyondTheCapEarnASummary() throws {
        // `newCount` is the pass's true total and `messages` is capped by the core.
        let notices = pass([
            account(
                newCount: 5,
                messages: [message(subject: "One", key: "m1"), message(subject: "Two", key: "m2")]
            )
        ])
        try #require(notices.count == 3)
        let summary = notices[2]
        // Its own copy, never the unknown-sender line: a single hidden message must not read as
        // one more subject-less message.
        #expect(summary.title == L10n.notification_more_messages(count: 3))
        #expect(summary.body == "account@example.test")
        #expect(summary.identifier == "new-mail-account-account")
        #expect(summary.threadIdentifier == "account")
    }

    @Test func aSummaryQuotesNoMessage() throws {
        // It stands for messages it does not name, so it has no one body to quote.
        let notices = pass([account(newCount: 3, messages: [message()])])
        try #require(notices.count == 2)
        #expect(notices[1].subtitle == "")
    }

    // MARK: every account on screen, and no dock count

    @Test func accountsAreReportedInOrderAndGroupedByTheirOwn() {
        let notices = pass([
            account(id: "a", messages: [message(key: "m1")]),
            account(id: "b", label: "other@example.test", messages: [message(key: "m2")]),
        ])
        #expect(notices.map(\.threadIdentifier) == ["a", "b"])
    }

    // MARK: what a click opens

    @Test func eachMessageCarriesTheMessageAClickOpens() throws {
        // The pair the core keys a message on, carried on the notification rather than recovered
        // from its identifier string.
        let notice = try only(message(key: "m7"))
        #expect(notice.target == NewMailTarget(account: "account", key: "m7"))
    }

    @Test func aSummaryCarriesNoMessageToOpen() throws {
        // It stands for messages it does not name, so a click on it brings the app forward and
        // leaves the reading pane where it was.
        let notices = pass([account(newCount: 3, messages: [message()])])
        try #require(notices.count == 2)
        #expect(notices[1].target == nil)
    }

    @Test func aClickTargetNamesItsOwnAccount() {
        let notices = pass([
            account(id: "a", messages: [message(key: "m1")]),
            account(id: "b", label: "other@example.test", messages: [message(key: "m2")]),
        ])
        #expect(notices.compactMap(\.target) == [
            NewMailTarget(account: "a", key: "m1"),
            NewMailTarget(account: "b", key: "m2"),
        ])
    }

    @Test func noNotificationSetsABadge() {
        // The pass total is what arrived just now, not what is unread, and a dock badge reading 3
        // and then 1 as passes land would say the second. Windows and Linux badge nothing either.
        let notices = pass([
            account(id: "a", newCount: 2, messages: [message(key: "m1"), message(key: "m2")]),
            account(id: "b", label: "other@example.test", messages: [message(key: "m3")]),
        ])
        #expect(notices.allSatisfy { $0.badge == nil })
    }
}
#endif
