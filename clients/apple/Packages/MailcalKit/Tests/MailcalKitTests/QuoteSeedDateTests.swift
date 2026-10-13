// A quote's attribution carries the original's date as the reading header shows it.
// `MailboxModel.composeContext(for:)` formats the core's instant once, with `localDateTime`, and
// hands the result to `ComposerQuote.words`, which must pass it through: running the formatter
// over it a second time is the failure this guards. `localDateTime` finds no `Z` and no `T` in
// `2026-08-31 07:01`, falls to `prefix(10)`, and the recipient gets a date with the time removed.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct QuoteSeedDateTests {
    private func original() -> QuoteSource {
        QuoteSource(
            from: "Sender <sender@example.test>",
            date: "2026-08-31T07:01:00Z",
            to: "recipient@example.test",
            cc: "",
            subject: "Planning",
            bodyHtml: "<p>Body</p>",
            bodyPlain: "Body"
        )
    }

    @Test func theAlreadyLocalisedDateReachesTheQuoteIntact() throws {
        let displayed = "2026-08-31 07:01"
        let words = ComposerQuote.words(original(), date: displayed, isForward: false)
        let sent = try #require(words.headers.first { $0.label == L10n.quote_sent() })

        #expect(words.line.contains(displayed), "the time was dropped from the attribution: \(words.line)")
        #expect(sent.value == displayed, "the time was dropped from the Sent header: \(sent.value)")
    }

    @Test func theHeaderBlockListsOnlyTheRecipientsTheOriginalHad() {
        let labels = ComposerQuote.words(original(), date: "", isForward: false).headers.map(\.label)
        #expect(labels == [L10n.quote_from(), L10n.quote_sent(), L10n.quote_to(), L10n.quote_subject()])
    }

    @Test func aForwardIsMarkedRatherThanAttributed() {
        let words = ComposerQuote.words(original(), date: "2026-08-31 07:01", isForward: true)
        #expect(words.line == L10n.quote_forwarded())
    }
}
