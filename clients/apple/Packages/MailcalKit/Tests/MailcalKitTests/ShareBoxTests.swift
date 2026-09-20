// The drop box a share is handed over in (docs/os-integration.md).
//
// What a share MEANS is the shared core's and is held by its Rust tests. What is Apple's, and what
// is covered here, is the hand-off itself: the extension and the app are two processes that never
// speak, so every rule about what one leaves and the other finds has to hold on its own.
//
// Built over a temporary directory rather than the real App Group, which no test bundle is
// entitled to reach; `ShareBox` takes its container for exactly that reason.

import Foundation
import MailcalShareBox
import Testing

@Suite struct ShareBoxTests {

    private func box() throws -> ShareBox {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("mailcal-sharebox-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return ShareBox(container: directory)
    }

    private func drop(_ name: String, at path: String = "/staged/file") -> ShareDrop {
        ShareDrop(
            files: [
                SharedFileRecord(path: path, suggestedName: name, declaredMediaType: "image/png")
            ],
            text: "",
            subject: name
        )
    }

    @Test func aDepositedShareComesBackWholeAndOnlyOnce() throws {
        let box = try box()
        try box.deposit(drop("holiday.png"))

        let taken = try #require(box.take())
        #expect(taken.files.count == 1)
        #expect(taken.files[0].suggestedName == "holiday.png")
        #expect(taken.files[0].declaredMediaType == "image/png")
        #expect(taken.subject == "holiday.png")
        // Taken means spent: the app opens one composer, and draining again on the next activation
        // must not open a second one for the same share.
        #expect(box.take() == nil)
    }

    @Test func sharesComeBackOldestFirst() throws {
        let box = try box()
        try box.deposit(drop("first.png"))
        // The notes are ordered by their file's modification time, which one filesystem tick can
        // collapse; the sleep is what makes the two distinguishable at all.
        Thread.sleep(forTimeInterval: 0.05)
        try box.deposit(drop("second.png"))

        #expect(box.take()?.subject == "first.png")
        #expect(box.take()?.subject == "second.png")
    }

    @Test func aNoteThatCannotBeReadIsThrownAwayRatherThanRetried() throws {
        let box = try box()
        let inbox = box.container.appendingPathComponent("share-inbox", isDirectory: true)
        try FileManager.default.createDirectory(at: inbox, withIntermediateDirectories: true)
        try "not json".write(
            to: inbox.appendingPathComponent("broken.json"), atomically: true, encoding: .utf8)
        try box.deposit(drop("holiday.png"))

        // The real share behind the broken note still arrives, and the broken one is gone: left
        // in place it would be reconsidered on every activation the app ever has.
        #expect(box.take()?.subject == "holiday.png")
        #expect(box.take() == nil)
        #expect(try FileManager.default.contentsOfDirectory(atPath: inbox.path).isEmpty)
    }

    @Test func aHalfWrittenNoteIsNeverOffered() throws {
        // The app reads this directory on every activation, so a note is moved into place rather
        // than written there: mid-write, it must read as absent, not as corrupt.
        let box = try box()
        let inbox = box.container.appendingPathComponent("share-inbox", isDirectory: true)
        try FileManager.default.createDirectory(at: inbox, withIntermediateDirectories: true)
        try "{\"id\":\"half".write(
            to: inbox.appendingPathComponent("partial.writing"), atomically: true, encoding: .utf8)

        #expect(box.take() == nil)
    }

    @Test func sweepingKeepsWhatACompserMightStillSend() throws {
        let box = try box()
        let fresh = try box.stagingDirectory(for: "fresh")
        let stale = try box.stagingDirectory(for: "stale")
        try FileManager.default.setAttributes(
            [.modificationDate: Date(timeIntervalSinceNow: -ShareBox.retention - 60)],
            ofItemAtPath: stale.path)

        box.sweep()

        // The staged path is what Send reads, so a composer left open over lunch must still find
        // its file; only what no draft could plausibly still hold goes.
        #expect(FileManager.default.fileExists(atPath: fresh.path))
        #expect(!FileManager.default.fileExists(atPath: stale.path))
    }

    @Test func eachShareStagesIntoItsOwnDirectory() throws {
        let box = try box()
        let first = try box.stagingDirectory(for: "one")
        let second = try box.stagingDirectory(for: "two")

        // Sweeping or losing one share may never reach another's files, which is what separate
        // directories buy; two shares of a file with the same name also have to coexist.
        #expect(first != second)
    }

    @Test func aShareCarryingNothingIsRecognisedBeforeItIsWritten() throws {
        #expect(ShareDrop().isEmpty)
        #expect(!ShareDrop(text: "https://example.test").isEmpty)
        #expect(!drop("holiday.png").isEmpty)
    }

    @Test func theDoorbellCarriesNoPayload() {
        // It only has to bring the app up: the share itself is already in the box, and a URL that
        // carried it would be a URL a web page could forge (docs/os-integration.md).
        let doorbell = ShareHandoff.doorbell(appID: "eu.allodia.mailcal")
        #expect(doorbell?.scheme == "eu.allodia.mailcal.share")
        #expect(doorbell?.absoluteString == "eu.allodia.mailcal.share://shared")
    }

    @Test func theDoorbellSchemeIsNotTheSignInScheme() {
        // Sign-in redirects use the app id itself as a scheme. Claiming that one would route an
        // OAuth redirect to the app's URL hook, where today each is captured inside its own
        // ASWebAuthenticationSession and none can arrive.
        #expect(ShareHandoff.scheme(appID: "eu.allodia.mailcal") != "eu.allodia.mailcal")
    }
}
