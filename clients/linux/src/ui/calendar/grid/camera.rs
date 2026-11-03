//! The camera after an event's title on the grid, where the event has a video meeting
//! (`docs/conferencing.md`, rule 7). The grid is painted with cairo, so the theme's symbolic icon
//! is snapshotted in the title's own colour and its render node drawn into the cairo context.

use std::cell::OnceCell;

use gtk::{cairo, gdk, prelude::*};

use crate::ui::{calendar::paint::Rgb, icons};

/// Between the end of the title and the camera.
pub(super) const GAP: f64 = 3.0;

/// The theme's camera for the surface being painted, looked up on the first event that draws it,
/// so a paint with no meeting on it does no icon lookup at all.
pub(super) struct Camera {
    widget: gtk::Widget,
    icon: OnceCell<gtk::IconPaintable>,
}

impl Camera {
    /// The camera for `widget`'s display and scale, at the size the grid's titles are drawn at.
    pub(super) fn of(widget: &impl IsA<gtk::Widget>) -> Self {
        Self {
            widget: widget.clone().upcast(),
            icon: OnceCell::new(),
        }
    }

    fn icon(&self) -> &gtk::IconPaintable {
        self.icon.get_or_init(|| {
            let display = self.widget.display();
            crate::ui::icons::install(&display);
            gtk::IconTheme::for_display(&display).lookup_icon(
                icons::VIDEO_MEETING,
                &[],
                16,
                self.widget.scale_factor(),
                gtk::TextDirection::None,
                gtk::IconLookupFlags::empty(),
            )
        })
    }

    /// Draws the camera `size` units square with its top left at (`x`, `y`), in `color`.
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
