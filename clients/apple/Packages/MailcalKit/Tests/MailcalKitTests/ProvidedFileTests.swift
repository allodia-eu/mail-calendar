// A file an item provider holds, copied before the provider takes it back: what the Share Extension
// and the photo picker both do with a photograph (Sources/MailcalShareBox/ProvidedFile.swift).
//
// The providers here are built the way the system's are: the file arrives through a file
// representation, on a queue of the provider's own, and is deleted once the handler returns.

import Foundation
import MailcalShareBox
import Testing
import UniformTypeIdentifiers

@Suite struct ProvidedFileTests {

    @Test func theTypeAskedForIsTheFirstThatIsAFile() {
        // The Photos app lists types beside the photograph that hold no file; asking for one of
        // those loads nothing.
        #expect(
            ProvidedFile.fileType(["com.example.private.thumbnail", "public.jpeg", "public.heic"])
                == "public.jpeg")
        #expect(ProvidedFile.fileType(["public.heic", "public.jpeg"]) == "public.heic")
        #expect(ProvidedFile.fileType(["com.adobe.pdf"]) == "com.adobe.pdf")
    }

    @Test func aFolderIsNeverTheTypeAskedFor() {
        #expect(ProvidedFile.fileType([UTType.folder.identifier]) == nil)
        #expect(
            ProvidedFile.fileType([UTType.folder.identifier, "public.png"]) == "public.png")
        #expect(ProvidedFile.fileType([]) == nil)
    }

    @Test func aPhotographIsNamedAfterItselfWithTheExtensionItArrivedAs() {
        let delivered = URL(fileURLWithPath: "/tmp/provider-internal/9F1C.jpeg")
        #expect(ProvidedFile.name(suggested: "IMG_0042", delivered: delivered) == "IMG_0042.jpeg")
        #expect(
            ProvidedFile.name(suggested: "IMG_0042.JPEG", delivered: delivered) == "IMG_0042.JPEG")
        #expect(ProvidedFile.name(suggested: nil, delivered: delivered) == "9F1C.jpeg")
        #expect(ProvidedFile.name(suggested: "", delivered: delivered) == "9F1C.jpeg")
    }

    @Test @MainActor func aPhotographIsCopiedBeforeTheProviderDeletesIt() async throws {
        let directory = try temporaryDirectory()
        let provider = photograph(named: "IMG_0042", bytes: Data([0xFF, 0xD8, 0xFF, 0xE0]))

        let copied = try #require(await ProvidedFile.copy(provider, into: directory))

        #expect(copied.lastPathComponent == "IMG_0042.jpeg")
        #expect(copied.deletingLastPathComponent().standardizedFileURL
            == directory.standardizedFileURL)
        #expect(try Data(contentsOf: copied) == Data([0xFF, 0xD8, 0xFF, 0xE0]))
    }

    @Test @MainActor func twoPhotographsOfOneNameAreTwoFiles() async throws {
        let directory = try temporaryDirectory()
        let first = try #require(
            await ProvidedFile.copy(photograph(named: "IMG_0001", bytes: Data([1])), into: directory))
        let second = try #require(
            await ProvidedFile.copy(photograph(named: "IMG_0001", bytes: Data([2])), into: directory))

        #expect(first != second)
        #expect(second.lastPathComponent == "IMG_0001-2.jpeg")
    }

    @Test @MainActor func aProviderWithNoFileAnswersNothing() async throws {
        #expect(await ProvidedFile.copy(NSItemProvider(), into: try temporaryDirectory()) == nil)
    }

    // MARK: - Fixtures

    private func temporaryDirectory() throws -> URL {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("mailcal-provided-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}

/// A provider shaped like one the Photos app hands over: a type that holds no file listed first,
/// then the photograph as a file representation, delivered from a background queue under a name of
/// the provider's own and deleted as soon as the handler returns.
func photograph(named name: String, bytes: Data) -> NSItemProvider {
    let provider = NSItemProvider()
    provider.suggestedName = name
    provider.registerDataRepresentation(
        forTypeIdentifier: "com.example.private.thumbnail", visibility: .all
    ) { completion in
        completion(Data(), nil)
        return nil
    }
    provider.registerFileRepresentation(
        forTypeIdentifier: UTType.jpeg.identifier, fileOptions: [], visibility: .all
    ) { completion in
        DispatchQueue.global().async {
            let loan = URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("loan-\(UUID().uuidString).jpeg")
            try? bytes.write(to: loan)
            completion(loan, false, nil)
            try? FileManager.default.removeItem(at: loan)
        }
        return nil
    }
    return provider
}
