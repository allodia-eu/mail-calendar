// What the Share Extension makes of a share request (Sources/MailcalShareBox/ShareIntake.swift).
//
// The request is built the way the Photos app hands one over: one item, one provider per
// photograph, the picture only as a file representation. The share sheet itself and bringing the
// app forward are `SharePhotoTests`' (UITests), which drives Photos.

import Foundation
import MailcalShareBox
import Testing

@Suite struct ShareIntakeTests {

    private func box() throws -> ShareBox {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("mailcal-intake-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return ShareBox(container: directory)
    }

    @Test @MainActor func aSharedPhotographIsStagedInTheBox() async throws {
        let box = try box()
        let item = NSExtensionItem()
        item.attachments = [photograph(named: "IMG_0002", bytes: Data([0xFF, 0xD8, 0xFF]))]

        let drop = try #require(await ShareIntake.read(items: [item], into: box))

        #expect(drop.files.count == 1)
        let file = drop.files[0]
        #expect(file.suggestedName == "IMG_0002")
        #expect(file.declaredMediaType == "image/jpeg")
        // Staged inside the box, where the app can read it after the provider has taken its own
        // copy back.
        #expect(file.path.hasPrefix(box.container.path))
        #expect(try Data(contentsOf: URL(fileURLWithPath: file.path)) == Data([0xFF, 0xD8, 0xFF]))
    }

    @Test @MainActor func aPhotographThatCannotBeLoadedIsRecordedWithoutAPath() async throws {
        // Recorded rather than dropped, so the core can name it as refused.
        let provider = NSItemProvider()
        provider.suggestedName = "IMG_0003"
        provider.registerFileRepresentation(
            forTypeIdentifier: "public.jpeg", fileOptions: [], visibility: .all
        ) { completion in
            completion(nil, false, CocoaError(.fileReadNoSuchFile))
            return nil
        }
        let item = NSExtensionItem()
        item.attachments = [provider]

        let drop = try #require(await ShareIntake.read(items: [item], into: try box()))

        #expect(drop.files.map(\.suggestedName) == ["IMG_0003"])
        #expect(drop.files[0].path.isEmpty)
    }
}
