import MailcalBindings
import SwiftUI

#if canImport(AppKit)
    import AppKit
    private typealias PlatformImage = NSImage
#else
    import UIKit
    private typealias PlatformImage = UIImage
#endif

/// The circle drawn beside a person: their photo when one is known, else their monogram on the
/// colour the core picked for them.
///
/// **Only the shape is decided here.** The letters, the colour and whether there is a photo all
/// come from the core (`docs/avatars.md`), resolved per client, four clients would disagree
/// about whether a white letter is legible on a mid-green fill, exactly as they would about a
/// calendar chip's label.
struct AvatarView: View {
    let avatar: Avatar
    var diameter: CGFloat = 34

    @Environment(\.colorScheme) private var colorScheme
    @State private var photo: PlatformImage?

    private var swatch: Swatch { colorScheme == .dark ? avatar.dark : avatar.light }

    var body: some View {
        Circle()
            .fill(parseHexColor(swatch.background))
            .frame(width: diameter, height: diameter)
            .overlay { content }
            // The photo is drawn to the circle's edge, so it is clipped rather than inset.
            .clipShape(Circle())
            // Decoration. The row already announces the person's name, and a monogram is a
            // restatement of it, announcing this would make VoiceOver read a letter before
            // every sender. The person glyph says nothing at all.
            .accessibilityHidden(true)
            .task(id: avatar.imagePath) { await loadPhoto() }
    }

    @ViewBuilder
    private var content: some View {
        if let photo {
            Image(platformImage: photo)
                .resizable()
                .scaledToFill()
                .frame(width: diameter, height: diameter)
        } else if avatar.initials.isEmpty {
            // Neither a name nor an address. The core deliberately sends no placeholder text:
            // any word it chose would be untranslatable English, so the platform's own glyph
            // stands in.
            Image(systemName: "person.crop.circle")
                .foregroundStyle(parseHexColor(swatch.text))
        } else {
            Text(avatar.initials)
                .font(.system(size: diameter * 0.4, weight: .medium))
                .foregroundStyle(parseHexColor(swatch.text))
        }
    }

    private func loadPhoto() async {
        guard let path = avatar.imagePath else {
            photo = nil
            return
        }
        photo = await AvatarPhotoCache.shared.image(atPath: path)
    }
}

/// Decoded avatar photos, keyed by their file path.
///
/// The path is safe as a cache key precisely because the engine names the file by a hash of its
/// own contents: a changed photo is a changed path, so an entry can never go stale and nothing
/// needs invalidating.
private actor AvatarPhotoCache {
    static let shared = AvatarPhotoCache()

    private let cache = NSCache<NSString, PlatformImage>()

    /// The decoded image at `path`, or `nil` if it cannot be read.
    ///
    /// Decoding happens off the main actor: a list scrolls past dozens of these, and doing it
    /// inline would drop frames. The core has already checked the bytes are a raster image
    /// within a size cap, so this does not sniff again.
    func image(atPath path: String) -> PlatformImage? {
        let key = path as NSString
        if let cached = cache.object(forKey: key) { return cached }
        guard let image = PlatformImage(contentsOfFile: path) else { return nil }
        cache.setObject(image, forKey: key)
        return image
    }
}

extension Image {
    fileprivate init(platformImage: PlatformImage) {
        #if canImport(AppKit)
            self.init(nsImage: platformImage)
        #else
            self.init(uiImage: platformImage)
        #endif
    }
}

/// The unread marker on a desktop-layout row, and the space it reserves when read.
///
/// A **dot**, not the envelope glyph it replaces: the avatar now occupies the gutter, and two
/// symbols competing for the same job left the row saying "unread" twice. `docs/avatars.md` binds the
/// dot to desktop layouts only, the compact phone list has no room for it, and bold subject
/// and sender already carry unread there.
/// **Whether to draw one is not decided here**, see `ContentView.unreadDot(_:)`, which is the
/// only thing that should build this. The size class has to be read at the window level, and a
/// row is the wrong place to read it from.
struct UnreadDot: View {
    let unread: Bool

    var body: some View {
        Circle()
            .fill(unread ? Color.accentColor : Color.clear)
            .frame(width: 7, height: 7)
            // The row's own accessibility value carries read state; a second announcement of it
            // before every sender is noise.
            .accessibilityHidden(true)
    }
}
