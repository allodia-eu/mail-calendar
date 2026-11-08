//! The glyphs an event wears on the grid: the camera on an event with a video meeting
//! (`docs/conferencing.md`, rule 7) and the repeat arrows on an occurrence of a series
//! (`docs/calendar.md` §4). The grid is painted with cairo, so the theme's symbolic icon is
//! snapshotted in the title's own colour and its render node drawn into the cairo context.

use std::cell::OnceCell;

use gtk::{cairo, gdk, prelude::*};

use crate::ui::{calendar::paint::Rgb, icons};

/// One theme icon for the surface being painted, looked up on the first event that draws it, so a
/// paint with no such event on it does no icon lookup at all.
pub(super) struct Glyph {
    widget: gtk::Widget,
    name: &'static str,
    icon: OnceCell<gtk::IconPaintable>,
}

impl Glyph {
    fn of(widget: &gtk::Widget, name: &'static str) -> Self {
        Self {
            widget: widget.clone(),
            name,
            icon: OnceCell::new(),
        }
    }

    fn icon(&self) -> &gtk::IconPaintable {
        self.icon.get_or_init(|| {
            let display = self.widget.display();
            icons::install(&display);
            gtk::IconTheme::for_display(&display).lookup_icon(
                self.name,
                &[],
                16,
                self.widget.scale_factor(),
                gtk::TextDirection::None,
                gtk::IconLookupFlags::empty(),
            )
        })
    }

    /// Draws the glyph `size` units square with its top left at (`x`, `y`), in `color`.
    pub(super) fn draw(&self, context: &cairo::Context, x: f64, y: f64, size: f64, color: Rgb) {
        let snapshot = gtk::Snapshot::new();
        #[allow(clippy::cast_possible_truncation)]
        let rgba = gdk::RGBA::new(color.red as f32, color.green as f32, color.blue as f32, 1.0);
        self.icon()
            .snapshot_symbolic(&snapshot, size, size, &[rgba]);
        let Some(node) = snapshot.to_node() else {
            return;
        };
        let _ = context.save();
        context.translate(x, y);
        node.draw(context);
        let _ = context.restore();
    }
}

/// The camera and the repeat arrows for one surface.
pub(super) struct Glyphs {
    pub(super) camera: Glyph,
    pub(super) repeat: Glyph,
}

impl Glyphs {
    pub(super) fn of(widget: &impl IsA<gtk::Widget>) -> Self {
        let widget = widget.clone().upcast::<gtk::Widget>();
        Self {
            camera: Glyph::of(&widget, icons::VIDEO_MEETING),
            repeat: Glyph::of(&widget, icons::REPEATS),
        }
    }
}
