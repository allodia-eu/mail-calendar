// Which face the reading header draws while a message is opening.
//
// Pinning it here rather than through the UI is the point of keeping `readingHeaderAvatar` pure:
// the case that matters only exists for opens slower than the core's threshold, so driving it
// through a view would mean timing a real fetch. The bug it guards shipped on two platforms:
// the header took a `pending` snapshot's avatar, which is the core's "nobody", and blanked a
// circle the list row had already filled.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct ReadingHeaderTests {
    private let rowAvatar = Avatar(
        initials: "AL",
        light: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        dark: Swatch(background: "#4C6EF5", text: "#FFFFFF", border: "#3B5BDB"),
        imagePath: "/photos/alice.png"
    )

    /// The core's avatar for nobody: what a `pending` snapshot carries, since it resolves nothing.
    private let nobody = Avatar(
        initials: "",
        light: Swatch(background: "#8A8A8E", text: "#FFFFFF", border: "#8A8A8E"),
        dark: Swatch(background: "#8A8A8E", text: "#FFFFFF", border: "#8A8A8E"),
        imagePath: nil
    )

    private func snapshot(avatar: Avatar, pending: Bool) -> ReadingSnapshot {
        ReadingSnapshot(
            key: "m1",
            from: pending ? "" : "Alice <alice@example.test>",
            avatar: avatar,
            to: "",
            cc: "",
            bcc: "",
            html: pending ? nil : "<p>Body</p>",
            plain: nil,
            hasRemoteImages: false,
            loadError: false,
            attachments: [],
            invitation: nil,
            pending: pending
        )
    }

    @Test func nothingPublishedYetKeepsTheRowsFace() {
        #expect(readingHeaderAvatar(snapshot: nil, row: rowAvatar).imagePath == "/photos/alice.png")
    }

    /// The regression: a wait must not cost the reader the face they were already looking at.
    @Test func aPendingSnapshotKeepsTheRowsFace() {
        let drawn = readingHeaderAvatar(snapshot: snapshot(avatar: nobody, pending: true), row: rowAvatar)
        #expect(drawn.initials == "AL")
        #expect(drawn.imagePath == "/photos/alice.png")
    }

    /// Once a body lands its avatar is the answer, it can only differ by having found a photo.
    @Test func aResolvedBodyUpgradesTheFace() {
        let found = Avatar(
            initials: "AL",
            light: rowAvatar.light,
            dark: rowAvatar.dark,
            imagePath: "/photos/alice-from-contacts.png"
        )
        let drawn = readingHeaderAvatar(snapshot: snapshot(avatar: found, pending: false), row: rowAvatar)
        #expect(drawn.imagePath == "/photos/alice-from-contacts.png")
    }
}
