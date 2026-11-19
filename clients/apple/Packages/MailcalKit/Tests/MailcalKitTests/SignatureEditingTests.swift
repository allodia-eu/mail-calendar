// What Settings → Signatures does with the core's page besides drawing it (docs/signatures.md,
// "Changing the library"): an open editor acts on an outcome once and only on its own signature,
// Save is held by the core's name rule, a slot picker's positions map to the library, a refusal is
// worded from the catalog, and a picked picture is asked of the core's rule before it is read.

import Foundation
import MailcalBindings
import Testing
import UniformTypeIdentifiers

@testable import MailcalUI

private let idle = ControlState(enabled: true, running: false)

private func page(
    outcome: SignatureOutcome? = nil, save: ControlState = idle, library: [SignatureEntry] = []
) -> SignaturesPage {
    SignaturesPage(
        signatures: library, accounts: [], add: idle, newId: "new", save: save, outcome: outcome)
}

private func outcome(_ serial: UInt64, _ id: String, _ result: SignatureResult) -> SignatureOutcome {
    SignatureOutcome(serial: serial, id: id, kind: .save, result: result)
}

private func entry(_ id: String, _ name: String) -> SignatureEntry {
    SignatureEntry(id: id, name: name, edit: idle, delete: idle)
}

@Suite struct SignatureEditorSessionTests {
    @Test func anOutcomeInForceWhenTheEditorOpenedIsNotItsOwn() {
        let before = outcome(4, "s1", .refused(reason: .blankName))
        var session = SignatureEditorSession(id: "s1", page: page(outcome: before))
        #expect(session.take(before) == nil)
    }

    @Test func aSaveOrAWriteThatFailedClosesTheEditor() {
        var saved = SignatureEditorSession(id: "s1", page: page())
        #expect(saved.take(outcome(1, "s1", .saved)) == .close)
        var unwritten = SignatureEditorSession(id: "s1", page: page())
        #expect(unwritten.take(outcome(1, "s1", .notWritten)) == .close)
    }

    @Test func aRefusalKeepsTheEditorOpenOnceForEachSave() {
        var session = SignatureEditorSession(id: "new", page: page())
        let refused = outcome(2, "new", .refused(reason: .image(refusal: .format)))
        #expect(session.take(refused) == .refused(.image(refusal: .format)))
        // The page is read again on every signal; the same outcome is not acted on twice.
        #expect(session.take(refused) == nil)
        #expect(session.take(outcome(3, "new", .saved)) == .close)
    }

    @Test func anotherSignaturesOutcomeIsNotActedOn() {
        var session = SignatureEditorSession(id: "s1", page: page())
        #expect(session.take(outcome(1, "s2", .saved)) == nil)
        #expect(session.take(nil) == nil)
    }

    @Test func saveIsHeldWhileTheCoreRefusesTheNameTyped() {
        #expect(SignatureEditorSession.save(page(), name: "Work").enabled)
        #expect(!SignatureEditorSession.save(page(), name: "").enabled)
        #expect(!SignatureEditorSession.save(page(), name: "   ").enabled)
        // A name with spaces around it is the core's to trim, not a refusal.
        #expect(SignatureEditorSession.save(page(), name: " Work ").enabled)
    }

    @Test func saveRunsWhileTheCoreSaves() {
        let running = ControlState(enabled: false, running: true)
        let save = SignatureEditorSession.save(page(save: running), name: "Work")
        #expect(save.running)
        #expect(!save.enabled)
    }
}

@Suite struct SignatureSlotChoicesTests {
    private let library = [entry("a", "Work"), entry("b", "Personal")]

    @Test func noneComesFirstThenTheLibraryInItsOrder() {
        #expect(
            SignatureSlotChoices.labels(library)
                == [L10n.settings_signatures_none(), "Work", "Personal"])
    }

    @Test func eachPositionNamesItsSignature() {
        #expect(SignatureSlotChoices.position(of: nil, in: library) == 0)
        #expect(SignatureSlotChoices.position(of: "b", in: library) == 2)
        #expect(SignatureSlotChoices.signature(at: 0, in: library) == nil)
        #expect(SignatureSlotChoices.signature(at: 1, in: library) == "a")
        #expect(SignatureSlotChoices.signature(at: 2, in: library) == "b")
    }
}

@Suite struct SignatureLabelsTests {
    @Test func eachRefusalIsWorded() {
        #expect(SignatureLabels.refusal(.blankName) == L10n.settings_signatures_name_required())
        #expect(SignatureLabels.refusal(.gone) == L10n.settings_signatures_gone())
        #expect(
            SignatureLabels.refusal(.image(refusal: .format))
                == L10n.settings_signatures_image_format())
    }

    @Test func theLimitIsTheOneTheCoreNames() {
        let limit = ByteCountFormatter.string(fromByteCount: 2_000_000, countStyle: .file)
        #expect(
            SignatureLabels.image(.tooLarge(limitBytes: 2_000_000))
                == L10n.settings_signatures_image_too_large(limit: limit))
        #expect(
            SignatureLabels.image(.tooLarge(limitBytes: 2_000_000))
                != SignatureLabels.image(.tooLarge(limitBytes: 524_288)))
    }
}

@Suite struct SignatureImageTests {
    /// A file of `size` bytes named `name`, in a directory of its own.
    private func file(_ name: String, size: Int) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent(name)
        try Data(repeating: 0x89, count: size).write(to: url)
        return url
    }

    @Test func thePickersOfferTheThreeTypesTheCoreAllows() {
        #expect(SignatureImage.types == [.png, .jpeg, .gif])
    }

    @Test func anAllowedPictureIsInsertedAsADataURL() throws {
        let url = try file("logo.png", size: 16)
        guard case let .dataURL(dataURL) = SignatureImage.load(url) else {
            Issue.record("a small PNG is inserted")
            return
        }
        #expect(dataURL.hasPrefix("data:image/png;base64,"))
    }

    @Test func aTypeTheCoreRefusesIsRefusedAsTheFormat() throws {
        let url = try file("logo.svg", size: 16)
        #expect(SignatureImage.load(url) == .refused(.format))
    }

    @Test func aFileTooLargeIsRefusedBeforeItIsRead() throws {
        // Unreadable, so only a refusal decided from its size and type answers anything but
        // `.failed`.
        let url = try file("logo.jpg", size: 600 * 1024)
        try FileManager.default.setAttributes([.posixPermissions: 0], ofItemAtPath: url.path)
        defer {
            try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        }
        guard case .refused(.tooLarge) = SignatureImage.load(url) else {
            Issue.record("a picture over the core's limit is refused by its size")
            return
        }
    }

    @Test func aFileThatCannotBeReadFails() throws {
        let url = try file("logo.gif", size: 16)
        try FileManager.default.setAttributes([.posixPermissions: 0], ofItemAtPath: url.path)
        defer {
            try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        }
        #expect(SignatureImage.load(url) == .failed)
    }
}
