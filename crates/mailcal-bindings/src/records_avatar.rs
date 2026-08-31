//! The sender avatar a client draws at the leading edge of a row.
//!
//! Its own file because [`records`](crate::records) is close to the 500-line limit, and
//! because this is one self-contained contract: [`docs/avatars.md`](../../../docs/avatars.md).

use mailcal_viewmodel::Avatar as AppAvatar;

use crate::records_calendar::Swatch;

/// A sender's avatar: what to draw, and in what colour.
///
/// **The client decides the shape; the core decides everything else.** Circle, rounded square
/// and size are genuinely platform-native. Which letters, and whether they are legible on
/// their fill, are not; resolved per client, four clients disagree about whether white reads
/// on a mid-green, which is the same reason calendar contrast is resolved in the core.
///
/// **It is decoration, and must be hidden from assistive technology.** The row already
/// announces the sender's name; a monogram beside it makes Narrator and TalkBack read a
/// letter before every sender.
#[derive(Clone, uniffi::Record)]
pub struct Avatar {
    /// One or two letters to draw when there is no image. Empty when the row names nobody;
    /// draw the platform's own person glyph rather than an empty circle.
    pub initials: String,
    /// How to draw the monogram in a light theme.
    pub light: Swatch,
    /// How to draw it in a dark theme.
    pub dark: Swatch,
    /// An image file to draw instead of the monogram, when one is known.
    ///
    /// A path, not bytes: the file is already on disk in the engine's blob area, and every
    /// platform has a decoder that reads one without copying the image through this API. It
    /// is named by a hash of its own contents, so the name changes when the picture does and
    /// a client may cache against it indefinitely.
    ///
    /// The core has already checked that the file begins with PNG, JPEG, GIF or WebP magic
    /// bytes and is within a size cap: a provider's own `Content-Type` is remote content
    /// describing itself and is not the check. **SVG never reaches here**: it is
    /// script-capable, and nothing in `rendering-security.md` permits it near a client
    /// surface.
    pub image_path: Option<String>,
}

impl From<AppAvatar> for Avatar {
    fn from(value: AppAvatar) -> Self {
        Self {
            initials: value.initials,
            light: value.light.into(),
            dark: value.dark.into(),
            image_path: value.image_path,
        }
    }
}
