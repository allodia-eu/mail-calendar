// The square at the start of a composer's file tile (`docs/composer-layout.md`, rule 4): the file
// itself, shrunk, when the core offers it as a `preview`, and the attachment glyph otherwise.

import ImageIO
import SwiftUI

struct ComposerFileThumbnail: View {
    /// The file's path when the core judged it a picture, `nil` otherwise.
    let preview: String?
    let side: CGFloat

    @Environment(\.displayScale) private var displayScale
    @State private var picture: CGImage?

    var body: some View {
        ZStack {
            if let picture {
                Image(decorative: picture, scale: displayScale)
                    .resizable()
                    .scaledToFill()
            } else {
                Image(systemName: "doc")
                    .foregroundStyle(.secondary)
            }
        }
        .frame(width: side, height: side)
        .clipShape(RoundedRectangle(cornerRadius: 5))
        .accessibilityHidden(true)
        .task(id: preview) {
            picture = nil
            guard let preview else { return }
            // Twice the square's pixels: the picture fills the square, so its shorter side is the
            // one that has to cover it.
            let pixels = Int(side * displayScale * 2)
            picture = await Task.detached(priority: .utility) {
                Self.decode(preview, maxPixels: pixels)
            }.value
        }
    }

    /// The picture at `path`, decoded no larger than `maxPixels` on its longer side, or `nil` when
    /// it cannot be read, which leaves the glyph.
    nonisolated static func decode(_ path: String, maxPixels: Int) -> CGImage? {
        guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil)
        else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixels,
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }
}
