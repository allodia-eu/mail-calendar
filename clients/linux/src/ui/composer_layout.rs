//! The composer's one column (`docs/composer-layout.md`): a scrolled window around the header, the
//! notes, the files and the editor, in that order, with nothing inside it that scrolls vertically
//! on its own.
//!
//! The editor runs in its growing mode, so its page never scrolls and it reports the height its
//! content needs and where its caret is. The core decides both answers: the editor's height is
//! `composer_editor_height` of the content and the room the column leaves under everything above
//! it, and the column scrolls to `composer_caret_scroll`.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use adw::prelude::*;
use gtk::{glib, graphene};
use mailcal_bindings::{composer_caret_scroll, composer_editor_height};

/// What the editor's page reported last, in CSS pixels, which on GTK are logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Caret {
    top: f64,
    bottom: f64,
}

/// One composer's column.
pub(crate) struct ComposerColumn {
    scroller: gtk::ScrolledWindow,
    content: gtk::Box,
    editor: webkit6::WebView,
    /// The height the editor's content needs, from its last report. Kept, because the room
    /// changes on its own: a resized window, a file added above the message.
    content_px: Cell<f64>,
    /// A caret the column could not yet scroll to, because the editor had not grown to hold it.
    caret: Cell<Option<Caret>>,
    /// Whether a resize of the editor is already waiting for the main loop.
    sizing: Cell<bool>,
    /// The handler on the adjustment, disconnected as the column goes.
    watch: RefCell<Option<glib::SignalHandlerId>>,
}

impl ComposerColumn {
    /// Wraps `content`, which ends with `editor`, in the column's one scroll.
    pub(crate) fn new(content: &gtk::Box, editor: &webkit6::WebView) -> Rc<Self> {
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroller.set_vscrollbar_policy(gtk::PolicyType::Automatic);
        scroller.set_vexpand(true);
        scroller.set_hexpand(true);
        scroller.set_child(Some(content));
        // The editor is exactly as tall as the core says; it takes no share of spare room itself.
        editor.set_vexpand(false);
        let column = Rc::new(Self {
            scroller,
            content: content.clone(),
            editor: editor.clone(),
            content_px: Cell::new(0.0),
            caret: Cell::new(None),
            sizing: Cell::new(false),
            watch: RefCell::new(None),
        });
        // The adjustment changes after every allocation that moves the column: the window
        // resized, a row added above the message, the editor grown. Weak, because the scroller
        // this handler hangs off is the column's own.
        let weak = Rc::downgrade(&column);
        let watch = column.scroller.vadjustment().connect_changed(move |_| {
            if let Some(column) = weak.upgrade() {
                column.queue_sizing();
                column.follow_caret();
            }
        });
        column.watch.replace(Some(watch));
        column
    }

    pub(crate) fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scroller
    }

    /// The editor's `height` notice.
    pub(crate) fn content_height(&self, px: u32) {
        self.content_px.set(f64::from(px));
        self.size_editor();
    }

    /// The editor's `caret` notice. Scrolled to once the editor is tall enough to hold it, which
    /// for a line just typed past the end is after the resize its `height` notice asked for.
    pub(crate) fn caret(&self, top: u32, bottom: u32) {
        self.caret.set(Some(Caret {
            top: f64::from(top),
            bottom: f64::from(bottom),
        }));
        self.follow_caret();
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

    fn size_editor(&self) {
        let Some(top) = self.editor_top() else {
            return;
        };
        let visible = self.scroller.vadjustment().page_size();
        let room = visible - top - f64::from(self.content.margin_bottom());
        let height = composer_editor_height(self.content_px.get(), room).ceil();
        // Whole logical pixels; the core's answer is never below zero.
        #[allow(clippy::cast_possible_truncation)]
        let height = height.min(f64::from(i32::MAX)) as i32;
        if self.editor.height_request() != height {
            self.editor.set_height_request(height);
        }
    }

    /// Scrolls the column to the caret the editor last reported. A caret the column is not yet
    /// long enough to reach is kept, and tried again after each allocation, until the editor has
    /// grown to hold it: while typing runs ahead of the resizes, several can be in flight. It is
    /// always reached in the end, because the editor is at least as tall as its content
    /// (`composer_editor_height`), and the caret's margin is less than the editor's own padding
    /// and the column's bottom margin together.
    fn follow_caret(&self) {
        let Some(caret) = self.caret.get() else {
            return;
        };
        let Some(top) = self.editor_top() else {
            return;
        };
        let adjustment = self.scroller.vadjustment();
        let target = composer_caret_scroll(
            adjustment.value(),
            adjustment.page_size(),
            top + caret.top,
            top + caret.bottom,
        );
        if let Some(target) = target {
            adjustment.set_value(target);
            // Half a pixel either way is the editor's height rounded to whole pixels.
            if target > adjustment.upper() - adjustment.page_size() + 0.5 {
                return;
            }
        }
        self.caret.set(None);
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
        if let Some(watch) = self.watch.take() {
            self.scroller.vadjustment().disconnect(watch);
        }
    }
}
