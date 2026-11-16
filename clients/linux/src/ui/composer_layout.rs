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
use gtk::{gio, glib, graphene};
use mailcal_bindings::{composer_caret_scroll, composer_editor_height};
use webkit6::prelude::WebViewExt;

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
    /// Whether the page has parsed and been switched to its growing mode, so its hooks exist.
    ready: Cell<bool>,
    /// The visible top the page was last told, and whether that call is still out.
    told: Cell<Option<f64>>,
    telling: Cell<bool>,
    /// The handlers on the adjustment, disconnected as the column goes.
    watches: RefCell<Vec<glib::SignalHandlerId>>,
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
            ready: Cell::new(false),
            told: Cell::new(None),
            telling: Cell::new(false),
            watches: RefCell::new(Vec::new()),
        });
        // The adjustment changes after every allocation that moves the column: the window
        // resized, a row added above the message, the editor grown. Weak, because the scroller
        // this handler hangs off is the column's own.
        let adjustment = column.scroller.vadjustment();
        let resized = Rc::downgrade(&column);
        let changed = adjustment.connect_changed(move |_| {
            if let Some(column) = resized.upgrade() {
                column.queue_sizing();
                column.follow_caret();
                column.tell_visible_top();
            }
        });
        let moved = Rc::downgrade(&column);
        let on_scroll = adjustment.connect_value_changed(move |_| {
            if let Some(column) = moved.upgrade() {
                column.tell_visible_top();
            }
        });
        column.watches.replace(vec![changed, on_scroll]);
        column
    }

    /// The page has parsed and runs in its growing mode, so it can be told where it is seen from.
    pub(crate) fn page_ready(self: &Rc<Self>) {
        self.ready.set(true);
        self.told.set(None);
        self.tell_visible_top();
    }

    /// Tells the page where the column's visible part starts in its own coordinates, so it keeps
    /// its toolbar there (`docs/composer-layout.md`, rule 7). One call at a time, carrying the
    /// latest value when it goes; the adjustment changes whenever the column scrolls or anything
    /// above the editor changes height.
    fn tell_visible_top(self: &Rc<Self>) {
        if !self.ready.get() || self.telling.get() {
            return;
        }
        let Some(top) = self.editor_top() else {
            return;
        };
        // Zero or less is the same answer to the page: its top is in view.
        let visible = visible_top(self.scroller.vadjustment().value(), top);
        if self.told.get() == Some(visible) {
            return;
        }
        self.told.set(Some(visible));
        self.telling.set(true);
        let weak: Weak<Self> = Rc::downgrade(self);
        self.editor.evaluate_javascript(
            &format!("window.setComposerVisibleTop({visible})"),
            None,
            None,
            None::<&gio::Cancellable>,
            move |_| {
                if let Some(column) = weak.upgrade() {
                    column.telling.set(false);
                    // The column may have moved on while the call was out.
                    column.tell_visible_top();
                }
            },
        );
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
        let adjustment = self.scroller.vadjustment();
        for watch in self.watches.take() {
            adjustment.disconnect(watch);
        }
    }
}

/// Where the column's visible part starts in the page's coordinates, in whole pixels: its scroll
/// offset less where the page starts in it, and zero while the page's top is in view.
pub(crate) fn visible_top(offset: f64, page_top: f64) -> f64 {
    (offset - page_top).max(0.0).round()
}
