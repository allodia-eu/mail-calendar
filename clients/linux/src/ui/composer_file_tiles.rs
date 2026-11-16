//! The files attached to a composer's message, drawn as tiles from the core's list
//! (`docs/composer-layout.md`, rules 3 and 4).
//!
//! The core holds the list. A tile is drawn from what it answers and nothing is kept here: the
//! picker, a drop and a paste add through [`FileTiles::add`], a tile's Remove takes its file off,
//! and each redraws from the list the core holds. Every save and the send read the same list in
//! the core, so the file on screen and the file that goes out cannot differ.
//!
//! An add reads each file's size and first bytes, which on a network mount can take seconds, so
//! it runs on a worker (`docs/waiting.md`); a remove and a read only take the core's lock.

use std::{
    rc::{Rc, Weak},
    sync::Arc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::{
    ComposerFileAttachment, ComposerFileTile, ComposerFiles, MailcalApp, MailcalError,
};

use super::{composer_file_preview, controls, icons};
use crate::l10n;

/// The tile's width, which is also its shortest: a name narrower than this is middle-truncated.
/// The flow box lays tiles out in as many equal columns of it as the composer's width holds.
const TILE_WIDTH: i32 = 220;
/// The most columns a wide window gets, so a tile never stretches past a readable width.
const MOST_COLUMNS: u32 = 6;

/// The core's three calls on a composition's files. A trait so the tiles and the routes into them
/// can be built in a test with no core behind them; `Send` and `Sync`, because an add runs on a
/// worker.
pub(crate) trait ComposerFileStore: Send + Sync {
    fn files(&self, composition: String) -> Result<ComposerFiles, MailcalError>;
    fn add(
        &self,
        composition: String,
        files: Vec<ComposerFileAttachment>,
    ) -> Result<ComposerFiles, MailcalError>;
    fn remove(&self, composition: String, file: String) -> Result<ComposerFiles, MailcalError>;
}

impl ComposerFileStore for MailcalApp {
    fn files(&self, composition: String) -> Result<ComposerFiles, MailcalError> {
        self.composer_files(composition)
    }

    fn add(
        &self,
        composition: String,
        files: Vec<ComposerFileAttachment>,
    ) -> Result<ComposerFiles, MailcalError> {
        self.add_composer_files(composition, files)
    }

    fn remove(&self, composition: String, file: String) -> Result<ComposerFiles, MailcalError> {
        self.remove_composer_file(composition, file)
    }
}

/// One composer's file tiles, under the line that counts them.
pub(crate) struct FileTiles {
    store: Option<Arc<dyn ComposerFileStore>>,
    composition: String,
    block: gtk::Box,
    summary: gtk::Label,
    flow: gtk::FlowBox,
}

impl FileTiles {
    /// The tiles for `composition`, drawn from what the core holds for it now: a forward's
    /// originals, a stored draft's own files, a share's, or what a composer it was moved from had.
    pub(crate) fn new(store: Option<Arc<dyn ComposerFileStore>>, composition: &str) -> Rc<Self> {
        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_homogeneous(true);
        flow.set_column_spacing(8);
        flow.set_row_spacing(8);
        flow.set_min_children_per_line(1);
        flow.set_max_children_per_line(MOST_COLUMNS);
        flow.set_valign(gtk::Align::Start);
        let summary = gtk::Label::new(None);
        summary.set_widget_name("composer-files-summary");
        summary.set_xalign(0.0);
        summary.add_css_class("caption");
        summary.add_css_class("dim-label");
        let block = gtk::Box::new(gtk::Orientation::Vertical, 6);
        block.append(&summary);
        block.append(&flow);
        let tiles = Rc::new(Self {
            store,
            composition: composition.to_owned(),
            block,
            summary,
            flow,
        });
        tiles.draw(&tiles.current());
        tiles
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.block
    }

    /// Attaches `files` after what the composition holds, on a worker, and redraws once they
    /// are in. The redraw reads the list as it stands then rather than the add's own answer: two
    /// adds can finish in either order, and the later answer drawn last is the whole list.
    pub(crate) fn add(self: &Rc<Self>, files: Vec<ComposerFileAttachment>) {
        if files.is_empty() {
            return;
        }
        let Some(store) = &self.store else {
            return;
        };
        let store = Arc::clone(store);
        let composition = self.composition.clone();
        // Weak: a composer closed while the files are read has nothing to redraw.
        let tiles: Weak<Self> = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let added = gio::spawn_blocking(move || store.add(composition, files).is_ok()).await;
            if let (Ok(true), Some(tiles)) = (added, tiles.upgrade()) {
                tiles.draw(&tiles.current());
            }
        });
    }

    fn remove(self: &Rc<Self>, file: String) {
        let Some(store) = &self.store else {
            return;
        };
        if let Ok(answer) = store.remove(self.composition.clone(), file) {
            self.draw(&answer);
        }
    }

    /// The list as the core holds it now. A composer with no core holds none.
    pub(crate) fn current(&self) -> ComposerFiles {
        self.store
            .as_ref()
            .and_then(|store| store.files(self.composition.clone()).ok())
            .unwrap_or(ComposerFiles {
                tiles: Vec::new(),
                changed: false,
                total_size: 0,
            })
    }

    fn draw(self: &Rc<Self>, files: &ComposerFiles) {
        self.flow.remove_all();
        for tile in &files.tiles {
            self.flow.append(&self.tile(tile));
        }
        self.summary.set_text(&summary_words(files));
        // An empty block still takes its row's spacing in the column.
        self.block.set_visible(!files.tiles.is_empty());
    }

    fn tile(self: &Rc<Self>, tile: &ComposerFileTile) -> gtk::FlowBoxChild {
        let name = gtk::Label::new(Some(&tile.file_name));
        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_width_chars(1);
        // A file that could not be read has no size to give, and the send says so if it still
        // cannot be read then.
        let size = tile.size.map(size_words);

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_widget_name("composer-file");
        row.add_css_class("card");
        row.set_size_request(TILE_WIDTH, -1);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        inner.set_margin_top(4);
        inner.set_margin_bottom(4);
        inner.set_margin_start(6);
        inner.set_margin_end(4);
        inner.set_hexpand(true);
        inner.append(&composer_file_preview::square(tile.preview.as_deref()));
        inner.append(&name);
        if let Some(size) = &size {
            let label = gtk::Label::new(Some(size));
            label.add_css_class("caption");
            label.add_css_class("dim-label");
            inner.append(&label);
        }
        let owner: Weak<Self> = Rc::downgrade(self);
        let id = tile.id.clone();
        let remove = controls::icon_button(icons::REMOVE, l10n::action_remove(), tile.remove, {
            move || {
                if let Some(tiles) = owner.upgrade() {
                    tiles.remove(id.clone());
                }
            }
        });
        inner.append(&remove);
        row.append(&inner);
        // The whole name, which the tile may have had to shorten.
        row.set_tooltip_text(Some(&tile.file_name));
        row.set_accessible_role(gtk::AccessibleRole::Group);
        row.update_property(&[gtk::accessible::Property::Label(&tile.file_name)]);
        if let Some(size) = &size {
            row.update_property(&[gtk::accessible::Property::Description(size)]);
        }
        // Nothing on a tile is chosen by moving to it: Tab goes straight to its Remove.
        let child = gtk::FlowBoxChild::new();
        child.set_focusable(false);
        child.set_child(Some(&row));
        child
    }
}

/// A file's size in the user's own units and decimal mark, as GLib words it for every GNOME app.
fn size_words(bytes: u64) -> String {
    gtk::glib::format_size(bytes).to_string()
}

/// How many files there are and what they come to together (`compose_files_summary`).
fn summary_words(files: &ComposerFiles) -> String {
    let count = i64::try_from(files.tiles.len()).unwrap_or(i64::MAX);
    l10n::compose_files_summary(count, &size_words(files.total_size))
}

#[cfg(test)]
#[path = "composer_file_tiles_tests.rs"]
pub(crate) mod tests;
