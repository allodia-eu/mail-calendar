//! The composer's column (`docs/composer-layout.md`): a scrolled window around the header, the
//! notes, the files and the editor, in that order.
//!
//! The editor runs in its framed mode: its box is exactly as tall as the column's visible part,
//! and the message scrolls inside it under a toolbar the page never moves. Nothing goes under the
//! editor, so the column's end is where the box fills the visible part. Down, the column carries
//! the header away first; up, the message goes back to its top before the header returns. Which
//! of the two a step moves is the core's answer, `composer_scroller`.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use adw::prelude::*;
use gtk::{gdk, glib, graphene};
use mailcal_bindings::{ComposerScroller, composer_caret_scroll, composer_scroller};
use webkit6::prelude::WebViewExt;

/// The shortest the message's box is drawn, before the column has measured its visible part.
const MINIMUM_EDITOR_HEIGHT: f64 = 240.0;

/// One composer's column.
pub(crate) struct ComposerColumn {
    scroller: gtk::ScrolledWindow,
    editor: webkit6::WebView,
    /// How far the message is scrolled inside its box, from the page's last `scroll` notice.
    message_offset: Cell<f64>,
    /// Whether a resize of the editor is already waiting for the main loop.
    sizing: Cell<bool>,
    /// The handler on the adjustment, disconnected as the column goes.
    watches: RefCell<Vec<glib::SignalHandlerId>>,
}

impl ComposerColumn {
    /// Wraps `content`, which ends with `host` holding `editor`, in the column's scroll.
    pub(crate) fn new(content: &gtk::Box, host: &gtk::Box, editor: &webkit6::WebView) -> Rc<Self> {
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        // It still scrolls, but the message's box draws the one scroll bar: one for the column
        // beside it would read as two scrolls.
        scroller.set_vscrollbar_policy(gtk::PolicyType::External);
        scroller.set_vexpand(true);
        scroller.set_hexpand(true);
        scroller.set_child(Some(content));
        // The editor is exactly as tall as the visible part; it takes no share of spare room.
        editor.set_vexpand(false);
        // The framed page draws no background, and neither does the view, so the toolbar sits on
        // the window's colour (rule 5).
        editor.set_background_color(&gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));
        let column = Rc::new(Self {
            scroller,
            editor: editor.clone(),
            message_offset: Cell::new(0.0),
            sizing: Cell::new(false),
            watches: RefCell::new(Vec::new()),
        });
        // The adjustment changes after every allocation that moves the column, the window resized
        // among them. Weak, because the scroller this handler hangs off is the column's own.
        let resized = Rc::downgrade(&column);
        let changed = column.scroller.vadjustment().connect_changed(move |_| {
            if let Some(column) = resized.upgrade() {
                column.queue_sizing();
            }
        });
        column.watches.replace(vec![changed]);
        column.hand_over_wheel(host);
        column
    }

    pub(crate) fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scroller
    }

    /// The editor's `scroll` notice.
    pub(crate) fn message_scrolled(&self, px: u32) {
        self.message_offset.set(f64::from(px));
    }

    /// The editor's `caret` notice, in the page's coordinates: the column scrolls by
    /// `composer_caret_scroll`, as far as its end. Past that the box fills the visible part, and
    /// the page keeps its caret in view inside it.
    pub(crate) fn caret(&self, top: u32, bottom: u32) {
        let Some(editor_top) = self.editor_top() else {
            return;
        };
        let adjustment = self.scroller.vadjustment();
        let target = composer_caret_scroll(
            adjustment.value(),
            adjustment.page_size(),
            editor_top + f64::from(top),
            editor_top + f64::from(bottom),
        );
        if let Some(target) = target {
            adjustment.set_value(target.min(column_end(&adjustment)));
        }
    }

    /// Hands each vertical step of a gesture over the editor to one scroll, the one the core
    /// names. The column's step is taken in the capture phase, before WebKit sees it. The
    /// message's goes on to WebKit and is stopped at `host` on its way back up: WebKit passes on
    /// a step it gave the page, so the column would otherwise move with the message. Only the
    /// vertical part is taken: a table wider than the message still scrolls sideways inside the
    /// page.
    fn hand_over_wheel(self: &Rc<Self>, host: &gtk::Box) {
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        // Weak: the editor holds the controller, and the column holds the editor.
        let weak: Weak<Self> = Rc::downgrade(self);
        wheel.connect_scroll(move |wheel, _, dy| {
            let Some(column) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if dy == 0.0 {
                return glib::Propagation::Proceed;
            }
            let adjustment = column.scroller.vadjustment();
            let end = column_end(&adjustment);
            let towards_end = dy > 0.0;
            match composer_scroller(
                adjustment.value(),
                end,
                column.message_offset.get(),
                towards_end,
            ) {
                ComposerScroller::Message => glib::Propagation::Proceed,
                ComposerScroller::Column => {
                    let by = step(
                        dy,
                        wheel.unit() == gdk::ScrollUnit::Wheel,
                        adjustment.page_size(),
                    );
                    adjustment.set_value((adjustment.value() + by).clamp(0.0, end));
                    glib::Propagation::Stop
                }
            }
        });
        self.editor.add_controller(wheel);
        let message = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        message.set_propagation_phase(gtk::PropagationPhase::Bubble);
        message.connect_scroll(|_, _, _| glib::Propagation::Stop);
        host.add_controller(message);
    }

    /// Sizes the editor on the next turn of the main loop, outside the allocation that noticed
    /// the change: a size request set during one is a resize GTK has to run again.
    fn queue_sizing(self: &Rc<Self>) {
        if self.sizing.replace(true) {
            return;
        }
        let weak: Weak<Self> = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(column) = weak.upgrade() {
                column.sizing.set(false);
                column.size_editor();
            }
        });
    }

    /// The editor's box is as tall as the column's visible part.
    fn size_editor(&self) {
        let visible = self.scroller.vadjustment().page_size();
        // Whole logical pixels of a window's height, so it fits an i32.
        #[allow(clippy::cast_possible_truncation)]
        let height = visible.max(MINIMUM_EDITOR_HEIGHT).ceil() as i32;
        if self.editor.height_request() != height {
            self.editor.set_height_request(height);
        }
    }

    /// Where the editor starts in the column's own coordinates: where it is in the visible part,
    /// plus how far the column has scrolled. `None` before the column is laid out.
    fn editor_top(&self) -> Option<f64> {
        let point = self
            .editor
            .compute_point(&self.scroller, &graphene::Point::new(0.0, 0.0))?;
        Some(f64::from(point.y()) + self.scroller.vadjustment().value())
    }
}

impl Drop for ComposerColumn {
    fn drop(&mut self) {
        let adjustment = self.scroller.vadjustment();
        for watch in self.watches.take() {
            adjustment.disconnect(watch);
        }
    }
}

/// The furthest the column scrolls, where the message's box fills its visible part.
fn column_end(adjustment: &gtk::Adjustment) -> f64 {
    (adjustment.upper() - adjustment.page_size()).max(0.0)
}

/// How far one step of a gesture moves the column, in logical pixels. A mouse wheel's detent is
/// GTK's own scrolled window step, the visible height to the power of two thirds; a touchpad's
/// step is already a distance.
pub(crate) fn step(dy: f64, wheel: bool, visible: f64) -> f64 {
    if wheel {
        dy * visible.max(0.0).powf(2.0 / 3.0)
    } else {
        dy
    }
}
