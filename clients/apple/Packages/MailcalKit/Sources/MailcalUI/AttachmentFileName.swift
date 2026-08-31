// The name of the temp copy an attachment is written to before it is handed to the OS.
//
// The extension is load-bearing rather than cosmetic: both `NSWorkspace.open` and Quick Look pick
// the viewer from it, so a PDF whose part carried the bare name `invoice` opens in nothing at all.
// Deriving it here keeps the rule testable, `swift test` runs on macOS, so it cannot reach the
// iOS presentation around it. See docs/rendering-security.md: hosts sanitise any name or extension
// they derive for a picker or temp path.

import Foundation
import UniformTypeIdentifiers

/// The file name for an attachment's temp copy: the sender's suggested name with path separators
/// neutralised, falling back to `attachment`, plus the media type's extension when the name
/// carries none. The derived extension comes from the system type database, so it needs no
/// sanitising of its own.
func attachmentFileName(name: String, mediaType: String) -> String {
    let fallback = "attachment"
    let sanitized = name.map { character -> Character in
        switch character {
        case "/", "\\", ":", "\0": return "_"
        default: return character
        }
    }
    let cleaned = String(sanitized).trimmingCharacters(in: .whitespacesAndNewlines)
    // A name of nothing but dots is `.` or `..`, a path component, not a file.
    let base = cleaned.isEmpty || cleaned.allSatisfy { $0 == "." } ? fallback : cleaned
    guard (base as NSString).pathExtension.isEmpty,
          let derived = UTType(mimeType: mediaType)?.preferredFilenameExtension
    else { return base }
    return "\(base).\(derived)"
}
