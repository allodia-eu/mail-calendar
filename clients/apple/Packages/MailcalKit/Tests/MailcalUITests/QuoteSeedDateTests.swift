// `OpenedMessage.date` is already localised on this platform: both constructions in
// `Mailcal.AutoAdvance.swift` run `localDateTime` before the message reaches a view. So the quote
// must pass it through, and running the formatter over it a second time is the failure this
// guards. `localDateTime` finds no `Z` and no `T` in `2026-08-31 07:01`, falls to `prefix(10)`,
// and the recipient gets a date with the time silently removed.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct QuoteSeedDateTests {
    private func avatar() -> Avatar {
        Avatar(
            initials: "S",
            light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
            dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
            imagePath: nil
        )
    }

    private func snapshot() -> ReadingSnapshot {
        ReadingSnapshot(
            key: "m1",
            from: "Sender <sender@example.test>",
            avatar: avatar(),
            to: "recipient@example.test",
            cc: "",
            bcc: "",
            html: "<p>Body</p>",
            plain: "Body",
            hasRemoteImages: false,
            loadError: false,
            attachments: [],
            invitation: nil,
            pending: false
        )
    }

    @Test func theAlreadyLocalisedDateReachesTheQuoteIntact() throws {
        // What `openedMessage` actually holds: localDateTime output, not an engine instant.
        let displayed = "2026-08-31 07:01"
        let message = OpenedMessage(
            account: "a0",
            key: "m1",
            subject: "Planning",
            from: "Sender <sender@example.test>",
            avatar: avatar(),
            date: displayed
        )
        let json = try #require(
            ComposerQuote.seedJSON(
                style: .indented,
                message: message,
                reading: snapshot(),
                isForward: false
            )
        )
        let payload = try #require(
            try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any]
        )
        let attribution = try #require(payload["attribution"] as? [String: Any])
        let line = try #require(attribution["line"] as? String)
        let headers = try #require(attribution["headers"] as? [[String: String]])
        let sent = try #require(headers[1]["value"])

        #expect(line.contains(displayed), "the time was dropped from the attribution: \(line)")
        #expect(sent == displayed, "the time was dropped from the Sent header: \(sent)")
    }
}
