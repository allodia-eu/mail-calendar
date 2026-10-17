// What a new-mail pass says on macOS (docs/background-sync.md), the host-testable half of the
// notification: the projection decides the words and the identities, `MailNotifier` only hands
// them to `UNUserNotificationCenter`. The Apple twin of the Linux client's `notification_parts`
// tests and Windows's `NewMailNoticesTests`.
//
// The words differ by platform: these are the Mac's, run by `swift test` on the host, and the
// phone's are `NewMailNoticesIOSTests`. The identities are the same on both, so they are outside
// the guard and run on an iOS simulator too (clients/apple/Scripts/test-kit.sh --ios).

import MailcalBindings
import Testing
import UserNotifications

@testable import MailcalUI

#if os(macOS)
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
        #expect(notices.map(\.identifier) == ["new-mail-7:account:m1", "new-mail-7:account:m2"])
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
}
#endif

/// Which notification replaces which, on both Apple platforms.
@Suite struct NewMailIdentityTests {

    private func account(_ id: String, keys: [String]) -> AccountNewMail {
        AccountNewMail(
            accountId: id,
            accountLabel: "\(id)@example.test",
            newCount: UInt32(keys.count),
            messages: keys.map {
                NewMailPreview(
                    sender: "jane@example.test", senderName: "Jane", subject: "Hello",
                    preview: "", received: "", messageKey: $0)
            }
        )
    }

    @Test func theSameKeyInTwoAccountsIsTwoNotifications() {
        // A provider key is unique only within its account: two IMAP accounts on the same server
        // software hand out the same one, and one identifier would let the second notification
        // replace the first.
        let notices = NewMailNotices.forPass(
            BackgroundSyncOutcome(
                accounts: [account("a", keys: ["1"]), account("b", keys: ["1"])], timedOut: false))
        #expect(notices.count == 2)
        #expect(Set(notices.map(\.identifier)).count == 2)
    }

    @Test func noTwoPairsSpellTheSameIdentifier() {
        // Joined with a bare separator, these two pairs would be one string.
        #expect(
            NewMailNotices.identifier(account: "a:b", key: "c")
                != NewMailNotices.identifier(account: "a", key: "b:c"))
    }

    @Test func aMessageIdentifierIsNeverASummarys() throws {
        // The summary is `new-mail-account-<id>`, and replaces only the account's previous one.
        let notices = NewMailNotices.forPass(
            BackgroundSyncOutcome(
                accounts: [
                    AccountNewMail(
                        accountId: "a", accountLabel: "a@example.test", newCount: 3,
                        messages: account("a", keys: ["account-a"]).messages)
                ],
                timedOut: false))
        try #require(notices.count == 2)
        #expect(notices[0].identifier != notices[1].identifier)
    }
    @Test func noNotificationSetsTheIconBadge() {
        // The icon carries the unified Inbox unread (`AppBadge`). A notification's badge would
        // overwrite it with the number one pass found.
        let notices = NewMailNotices.forPass(
            BackgroundSyncOutcome(
                accounts: [
                    AccountNewMail(
                        accountId: "a", accountLabel: "a@example.test", newCount: 3,
                        messages: account("a", keys: ["1"]).messages),
                    account("b", keys: ["2"]),
                ],
                timedOut: false))
        #expect(notices.count == 3)
        #expect(notices.allSatisfy { MailNotifier.content($0).badge == nil })
    }
}
