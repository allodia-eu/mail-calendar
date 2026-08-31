//! Contract-compliant sender and contact avatars for GTK surfaces.

use std::{cell::RefCell, f64::consts::TAU};

use adw::prelude::*;
use gtk::gdk::prelude::GdkCairoContextExt;
use mailcal_bindings::Avatar;

/// Client-owned avatar data, equatable so a photo arriving rebuilds only the affected row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AvatarData {
    pub(crate) initials: String,
    light: Swatch,
    dark: Swatch,
    image_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Swatch {
    background: String,
    text: String,
    border: String,
}

impl From<&Avatar> for AvatarData {
    fn from(value: &Avatar) -> Self {
        Self {
            initials: value.initials.clone(),
            light: Swatch {
                background: value.light.background.clone(),
                text: value.light.text.clone(),
                border: value.light.border.clone(),
            },
            dark: Swatch {
                background: value.dark.background.clone(),
                text: value.dark.text.clone(),
                border: value.dark.border.clone(),
            },
            image_path: value.image_path.clone(),
        }
    }
}

/// A replaceable avatar used by the reading header.
pub(crate) struct Slot {
    root: gtk::Box,
    rendered: RefCell<Option<AvatarData>>,
    size: i32,
}

impl Slot {
    pub(crate) fn new(size: i32) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_valign(gtk::Align::Start);
        root.set_visible(false);
        Self {
            root,
            rendered: RefCell::new(None),
            size,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub(crate) fn set(&self, avatar: &AvatarData) {
        if self.rendered.borrow().as_ref() == Some(avatar) {
            return;
        }
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.root.append(&view(avatar, self.size));
        self.root.set_visible(true);
        *self.rendered.borrow_mut() = Some(avatar.clone());
    }

    pub(crate) fn clear(&self) {
        self.root.set_visible(false);
        *self.rendered.borrow_mut() = None;
    }
}

/// Draws a circular photo or the core's monogram, hidden from assistive technology.
pub(crate) fn view(avatar: &AvatarData, size: i32) -> gtk::Widget {
    let pixbuf = avatar
        .image_path
        .as_deref()
        .and_then(|path| gtk::gdk_pixbuf::Pixbuf::from_file(path).ok());
    let glyph_fallback = avatar.initials.is_empty() && pixbuf.is_none();
    let area = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_width(size)
        .content_height(size)
        .build();
    let drawn = avatar.clone();
    area.set_draw_func(move |_, context, width, height| {
        draw(context, width, height, &drawn, pixbuf.as_ref());
    });
    let redraw = area.downgrade();
    adw::StyleManager::default().connect_dark_notify(move |_| {
        if let Some(redraw) = redraw.upgrade() {
            redraw.queue_draw();
        }
    });

    if glyph_fallback {
        let overlay = gtk::Overlay::builder()
            .accessible_role(gtk::AccessibleRole::Presentation)
            .child(&area)
            .build();
        let glyph = gtk::Image::builder()
            .accessible_role(gtk::AccessibleRole::Presentation)
            .icon_name("avatar-default-symbolic")
            .build();
        glyph.set_pixel_size(size / 2);
        overlay.add_overlay(&glyph);
        return overlay.upcast();
    }
    area.upcast()
}

/// The fixed desktop unread column: presentational dot, then avatar, then row text.
pub(crate) fn unread_dot(unread: bool) -> gtk::DrawingArea {
    let dot = gtk::DrawingArea::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .content_width(8)
        .content_height(8)
        .build();
    dot.set_draw_func(move |_, context, width, height| {
        if unread {
            context.set_source_rgb(0.13, 0.48, 0.84);
            context.arc(
                f64::from(width) / 2.0,
                f64::from(height) / 2.0,
                f64::from(width.min(height)) / 2.0,
                0.0,
                TAU,
            );
            let _ = context.fill();
        }
    });
    dot.set_valign(gtk::Align::Center);
    dot
}

fn draw(
    context: &gtk::cairo::Context,
    width: i32,
    height: i32,
    avatar: &AvatarData,
    pixbuf: Option<&gtk::gdk_pixbuf::Pixbuf>,
) {
    let swatch = if adw::StyleManager::default().is_dark() {
        &avatar.dark
    } else {
        &avatar.light
    };
    let center_x = f64::from(width) / 2.0;
    let center_y = f64::from(height) / 2.0;
    let radius = f64::from(width.min(height)) / 2.0 - 1.0;
    context.arc(center_x, center_y, radius, 0.0, TAU);
    if let Some(pixbuf) = pixbuf {
        let _ = context.save();
        context.clip();
        let scale = (f64::from(width) / f64::from(pixbuf.width()))
            .max(f64::from(height) / f64::from(pixbuf.height()));
        context.translate(
            (f64::from(width) - f64::from(pixbuf.width()) * scale) / 2.0,
            (f64::from(height) - f64::from(pixbuf.height()) * scale) / 2.0,
        );
        context.scale(scale, scale);
        context.set_source_pixbuf(pixbuf, 0.0, 0.0);
        let _ = context.paint();
        let _ = context.restore();
    } else {
        let background = rgb(&swatch.background).unwrap_or((0.35, 0.35, 0.35));
        context.set_source_rgb(background.0, background.1, background.2);
        let _ = context.fill();
        if !avatar.initials.is_empty() {
            let text = rgb(&swatch.text).unwrap_or((1.0, 1.0, 1.0));
            context.set_source_rgb(text.0, text.1, text.2);
            context.select_font_face(
                "sans",
                gtk::cairo::FontSlant::Normal,
                gtk::cairo::FontWeight::Bold,
            );
            context.set_font_size(f64::from(width.min(height)) * 0.38);
            if let Ok(extents) = context.text_extents(&avatar.initials) {
                context.move_to(
                    center_x - extents.width() / 2.0 - extents.x_bearing(),
                    center_y - extents.height() / 2.0 - extents.y_bearing(),
                );
                let _ = context.show_text(&avatar.initials);
            }
        }
    }
    border_path(context, center_x, center_y, radius);
    let edge = rgb(&swatch.border).unwrap_or((0.2, 0.2, 0.2));
    context.set_source_rgb(edge.0, edge.1, edge.2);
    context.set_line_width(2.0);
    let _ = context.stroke();
}

fn border_path(context: &gtk::cairo::Context, center_x: f64, center_y: f64, radius: f64) {
    context.new_path();
    context.arc(center_x, center_y, radius, 0.0, TAU);
}

fn rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let hex = hex.strip_prefix('#')?;
    (hex.len() == 6).then_some((
        f64::from(u8::from_str_radix(&hex[0..2], 16).ok()?) / 255.0,
        f64::from(u8::from_str_radix(&hex[2..4], 16).ok()?) / 255.0,
        f64::from(u8::from_str_radix(&hex[4..6], 16).ok()?) / 255.0,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use adw::prelude::*;
    use gtk::cairo::{Context, Format, ImageSurface, PathSegment};

    pub(crate) fn avatars_and_unread_dots_are_presentational() {
        let avatar = super::AvatarData::from(&crate::ui::model::blank_avatar());
        assert_eq!(
            super::view(&avatar, 36).accessible_role(),
            gtk::AccessibleRole::Presentation
        );
        assert_eq!(
            super::unread_dot(true).accessible_role(),
            gtk::AccessibleRole::Presentation
        );
    }

    #[test]
    fn avatar_border_does_not_continue_the_monogram_path() {
        let surface = ImageSurface::create(Format::ARgb32, 36, 36).expect("image surface");
        let context = Context::new(&surface).expect("Cairo context");
        context.move_to(18.0, 18.0);

        super::border_path(&context, 18.0, 18.0, 17.0);

        let first = context
            .copy_path()
            .expect("avatar border path")
            .iter()
            .next();
        assert_eq!(first, Some(PathSegment::MoveTo((35.0, 18.0))));
    }
}
