// The date a quoted reply carries is read by the *recipient*, so a raw UTC instant from the core
// is a defect in their mailbox rather than in ours. Every client localises it the way the reading
// header does (`docs/timestamps.md`).

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct QuoteSeedDateTests {
    private func message(date: String) -> OpenedMessage {
        OpenedMessage(
            account: "a0",
            key: "m1",
            subject: "Planning",
            from: "Sender <sender@example.test>",
            // Any avatar: what is under test is the date, not what it draws.
            avatar: Avatar(
                initials: "S",
                light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
                dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
                imagePath: nil
            ),
            date: date
        )
    }

    private func snapshot() -> ReadingSnapshot {
        ReadingSnapshot(
            key: "m1",
            from: "Sender <sender@example.test>",
            avatar: Avatar(
                initials: "S",
                light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
                dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
                imagePath: nil
            ),
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

    @Test func theQuotedDateIsLocalisedBeforeItIsSentToAnyone() throws {
        let json = try #require(
            ComposerQuote.seedJSON(
                style: .indented,
                message: message(date: "2026-08-31T05:01:00Z"),
                reading: snapshot(),
                isForward: false,
                zone: "Europe/Amsterdam"
            )
        )
        let payload = try #require(
            try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any]
        )
        let attribution = try #require(payload["attribution"] as? [String: Any])
        let line = try #require(attribution["line"] as? String)
        let headers = try #require(attribution["headers"] as? [[String: String]])
        let sent = try #require(headers[1]["value"])

        for value in [line, sent] {
            #expect(!value.contains("T"), "raw ISO instant reached the quote: \(value)")
            #expect(!value.contains("Z"), "raw ISO instant reached the quote: \(value)")
        }
        // Amsterdam is UTC+2 on 31 August, so the localised hour proves the instant was converted
        // rather than merely reformatted.
        #expect(line.contains("07:01"), "not converted to the display zone: \(line)")
        #expect(sent.contains("07:01"), "not converted to the display zone: \(sent)")
    }
}
