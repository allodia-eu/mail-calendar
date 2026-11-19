// What Settings → Signatures does with the core's page that is not drawing (docs/signatures.md,
// "Changing the library"): the words for a refusal, what an open editor does with the page's
// outcome, a slot picker's positions, and a picked picture asked of the core's rule before it is
// read. Plain types, so the unit suite reads them without a screen.

import Foundation
import MailcalBindings
import UniformTypeIdentifiers

/// The catalog's words for the core's answers.
enum SignatureLabels {
    /// Why a signature was not saved.
    static func refusal(_ refusal: SignatureRefusal) -> String {
        switch refusal {
        case .blankName: L10n.settings_signatures_name_required()
        case .gone: L10n.settings_signatures_gone()
        case let .image(picture): image(picture)
        }
    }

    /// Why a picture may not go into a signature.
    static func image(_ refusal: SignatureImageRefusal) -> String {
        switch refusal {
        case .format:
            L10n.settings_signatures_image_format()
        case let .tooLarge(limitBytes):
            L10n.settings_signatures_image_too_large(
                limit: ByteCountFormatter.string(
                    fromByteCount: Int64(clamping: limitBytes), countStyle: .file))
        }
    }
}

/// What an open editor does with a change's outcome.
enum SignatureEditorVerdict: Equatable {
    /// Saved, written or not, or deleted: the editor closes. A save that was not written is the
    /// failed-save notice's to say.
    case close
    /// Not saved: the editor stays open and says why.
    case refused(SignatureRefusal)
}

/// One editor's hold on the page: the signature it edits and the newest outcome it has seen.
struct SignatureEditorSession {
    /// The signature the editor saves: the entry's id, or the page's `newId` for a new one.
    let id: String
    private var seen: UInt64

    /// An editor opening on `id` now, which an outcome already on `page` does not concern.
    init(id: String, page: SignaturesPage) {
        self.id = id
        seen = page.outcome?.serial ?? 0
    }

    /// What to do with `outcome`, once: `nil` for one this editor has seen, or for another
    /// signature's.
    mutating func take(_ outcome: SignatureOutcome?) -> SignatureEditorVerdict? {
        guard let outcome, outcome.serial > seen else { return nil }
        seen = outcome.serial
        guard outcome.id == id else { return nil }
        switch outcome.result {
        case .saved, .notWritten, .deleted: return .close
        case let .refused(reason): return .refused(reason)
        }
    }

    /// The editor's Save: the page's, held besides while the core refuses the name typed.
    static func save(_ page: SignaturesPage, name: String) -> ControlState {
        page.save.held(signatureNameRefusal(name: name) != nil)
    }
}

/// A slot picker's positions: None first, then the library in its order.
enum SignatureSlotChoices {
    static func labels(_ library: [SignatureEntry]) -> [String] {
        [L10n.settings_signatures_none()] + library.map(\.name)
    }

    /// The position `selected` is drawn at.
    static func position(of selected: String?, in library: [SignatureEntry]) -> Int {
        guard let selected, let index = library.firstIndex(where: { $0.id == selected }) else {
            return 0
        }
        return index + 1
    }

    /// The signature at `position`, `nil` for None.
    static func signature(at position: Int, in library: [SignatureEntry]) -> String? {
        library.indices.contains(position - 1) ? library[position - 1].id : nil
    }
}

/// A picture picked for a signature, as a `data:` URI or the reason it cannot be one.
enum SignatureImage {
    enum Outcome: Equatable {
        /// A `data:image/…;base64,…` URI, ready to insert.
        case dataURL(String)
        /// The core's picture rule refuses it; nothing was read.
        case refused(SignatureImageRefusal)
        /// The file could not be read.
        case failed
    }

    /// The pickers offer these, which are what the core's rule allows.
    static let types: [UTType] = [.png, .jpeg, .gif]

    /// Asks the core's rule of the file's type and size, and reads the file only when it allows
    /// the picture.
    static func load(_ url: URL) -> Outcome {
        guard let values = try? url.resourceValues(forKeys: [.contentTypeKey, .fileSizeKey]),
              let size = values.fileSize
        else { return .failed }
        let type = values.contentType ?? UTType(filenameExtension: url.pathExtension)
        let mediaType = type?.preferredMIMEType ?? ""
        if let refusal = signatureImageRefusal(mediaType: mediaType, size: UInt64(clamping: size)) {
            return .refused(refusal)
        }
        guard let data = try? Data(contentsOf: url) else { return .failed }
        return .dataURL("data:\(mediaType);base64,\(data.base64EncodedString())")
    }
}
