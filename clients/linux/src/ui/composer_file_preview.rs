//! The square at the start of a composer's file tile: the file's own picture when the core offers
//! a `preview`, and the attachment glyph otherwise (`docs/composer-layout.md`, rule 4).
//!
//! The picture is decoded off the main thread at the square's size, never at its own: a forward
//! of twenty camera photos would otherwise decode twenty full-size images to draw twenty
//! thumbnails. The glyph is drawn first and stays when the decode fails.

use std::sync::Once;

use adw::prelude::*;
use gtk::{gdk, gdk_pixbuf::Pixbuf, gio, glib};

use super::icons;

/// The square's side, in logical pixels.
pub(crate) const SQUARE: i32 = 28;
/// The glyph's size inside it.
const GLYPH: i32 = 16;
/// What a test and the test agent find the decoded picture by.
pub(crate) const PICTURE_NAME: &str = "composer-file-preview";

const CSS: &str = "
.composer-file-square {
  border-radius: 6px;
  background-color: alpha(currentColor, 0.08);
}
";

/// The square for a tile whose core answer carries `preview`.
pub(crate) fn square(preview: Option<&str>) -> gtk::Box {
    install_styles();
    let square = gtk::Box::new(gtk::Orientation::Vertical, 0);
    square.add_css_class("composer-file-square");
    square.set_size_request(SQUARE, SQUARE);
    square.set_valign(gtk::Align::Center);
    square.set_overflow(gtk::Overflow::Hidden);
    // The name beside it says what the file is; the square adds nothing a screen reader needs.
    square.set_accessible_role(gtk::AccessibleRole::Presentation);
    let glyph = gtk::Image::from_icon_name(icons::ATTACHMENT);
    glyph.set_pixel_size(GLYPH);
    glyph.set_vexpand(true);
    glyph.set_valign(gtk::Align::Center);
    glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
    square.append(&glyph);
    if let Some(path) = preview {
        decode_into(&square, path.to_owned());
    }
    square
}

/// Decodes `path` on a worker thread and puts the picture in `square` in place of its glyph.
/// Weak, because a tile redrawn or a composer closed while the decode runs has nothing to show it.
fn decode_into(square: &gtk::Box, path: String) {
    // In device pixels, so the picture is as sharp as the screen draws it.
    let side = SQUARE * square.scale_factor().max(1);
    let weak = square.downgrade();
    glib::MainContext::default().spawn_local(async move {
        let Ok(Some(pixels)) = gio::spawn_blocking(move || decode(&path, side)).await else {
            return;
        };
        let Some(square) = weak.upgrade() else {
            return;
        };
        let picture = gtk::Picture::for_paintable(&pixels.texture());
        picture.set_widget_name(PICTURE_NAME);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        picture.set_size_request(SQUARE, SQUARE);
        picture.set_accessible_role(gtk::AccessibleRole::Presentation);
        while let Some(child) = square.first_child() {
            square.remove(&child);
        }
        square.append(&picture);
    });
}

/// A decoded picture, in a form that crosses threads: a `Pixbuf` does not.
struct Pixels {
    width: i32,
    height: i32,
    stride: usize,
    alpha: bool,
    bytes: glib::Bytes,
}

impl Pixels {
    fn texture(&self) -> gdk::Texture {
        let format = if self.alpha {
            gdk::MemoryFormat::R8g8b8a8
        } else {
            gdk::MemoryFormat::R8g8b8
        };
        gdk::MemoryTexture::new(self.width, self.height, format, &self.bytes, self.stride).upcast()
    }
}

/// The picture at `path`, scaled so its shorter side is `side` pixels, which covers the square
/// once cropped. `None` for anything the loaders cannot read.
fn decode(path: &str, side: i32) -> Option<Pixels> {
    let (_, width, height) = Pixbuf::file_info(path)?;
    let (width, height) = cover(width, height, side)?;
    let pixbuf = Pixbuf::from_file_at_scale(path, width, height, false).ok()?;
    // A phone's photo is stored as the sensor saw it and turned upright by its EXIF orientation;
    // without it a portrait photo shows on its side.
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
    Some(Pixels {
        width: pixbuf.width(),
        height: pixbuf.height(),
        stride: usize::try_from(pixbuf.rowstride()).ok()?,
        alpha: pixbuf.has_alpha(),
        bytes: pixbuf.read_pixel_bytes(),
    })
}

/// The size a `width` by `height` picture is decoded at so its shorter side is `side`, never
/// larger than it is: a small picture is drawn as it is rather than blown up first.
pub(crate) fn cover(width: i32, height: i32, side: i32) -> Option<(i32, i32)> {
    if width <= 0 || height <= 0 || side <= 0 {
        return None;
    }
    let shorter = width.min(height);
    if shorter <= side {
        return Some((width, height));
    }
    let scale = f64::from(side) / f64::from(shorter);
    // Both are at most the picture's own size, so they fit an i32.
    #[allow(clippy::cast_possible_truncation)]
    let scaled = |length: i32| ((f64::from(length) * scale).round() as i32).max(1);
    Some((scaled(width), scaled(height)))
}

fn install_styles() {
    static INSTALLED: Once = Once::new();
    if let Some(display) = gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }
}
