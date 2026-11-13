//! The files attached to a composer's message, drawn as tiles from the core's list
//! (`docs/composer-layout.md`, rules 3 and 4).
//!
//! The core holds the list. A tile is drawn from what it answers and nothing is kept here: the
//! picker, a drop and a paste add through [`FileTiles::add`], a tile's Remove takes its file off,
//! and each redraws from the list the call answered with. Every save and the send read the same
//! list in the core, so the file on screen and the file that goes out cannot differ.

use std::{
    rc::{Rc, Weak},
    sync::Arc,
};

use adw::prelude::*;
use mailcal_bindings::{
    ComposerFileAttachment, ComposerFileTile, ComposerFiles, MailcalApp, MailcalError,
};

use super::{controls, icons};
use crate::l10n;

/// The tile's width, which is also its shortest: a name narrower than this is middle-truncated.
/// The flow box lays tiles out in as many equal columns of it as the composer's width holds.
const TILE_WIDTH: i32 = 220;
/// The most columns a wide window gets, so a tile never stretches past a readable width.
const MOST_COLUMNS: u32 = 6;

/// The core's three calls on a composition's files. A trait so the tiles and the routes into them
/// can be built in a test with no core behind them.
pub(crate) trait ComposerFileStore {
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

/// One composer's file tiles.
pub(crate) struct FileTiles {
    store: Option<Arc<dyn ComposerFileStore>>,
    composition: String,
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
        let tiles = Rc::new(Self {
            store,
            composition: composition.to_owned(),
            flow,
        });
        tiles.draw(&tiles.current());
        tiles
    }

    pub(crate) fn widget(&self) -> &gtk::FlowBox {
        &self.flow
    }

    /// Attaches `files` after what the composition holds, and redraws.
    pub(crate) fn add(self: &Rc<Self>, files: Vec<ComposerFileAttachment>) {
        if files.is_empty() {
            return;
        }
        let Some(store) = &self.store else {
            return;
        };
        if let Ok(answer) = store.add(self.composition.clone(), files) {
            self.draw(&answer);
        }
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
            })
    }

    fn draw(self: &Rc<Self>, files: &ComposerFiles) {
        self.flow.remove_all();
        for tile in &files.tiles {
            self.flow.append(&self.tile(tile));
        }
        // An empty flow box still takes its row's spacing in the column.
        self.flow.set_visible(!files.tiles.is_empty());
    }

    fn tile(self: &Rc<Self>, tile: &ComposerFileTile) -> gtk::FlowBoxChild {
        let name = gtk::Label::new(Some(&tile.file_name));
        name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        name.set_xalign(0.0);
        name.set_hexpand(true);
        name.set_width_chars(1);
        let words = gtk::Box::new(gtk::Orientation::Vertical, 2);
        words.set_valign(gtk::Align::Center);
        words.append(&name);
        // A file that could not be read has no size to give, and the send says so if it still
        // cannot be read then.
        let size = tile.size.map(size_words);
        if let Some(size) = &size {
            let label = gtk::Label::new(Some(size));
            label.set_xalign(0.0);
            label.add_css_class("caption");
            label.add_css_class("dim-label");
            words.append(&label);
        }

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_widget_name("composer-file");
        row.add_css_class("card");
        row.set_size_request(TILE_WIDTH, -1);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inner.set_margin_top(6);
        inner.set_margin_bottom(6);
        inner.set_margin_start(12);
        inner.set_margin_end(6);
        inner.set_hexpand(true);
        inner.append(&words);
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

#[cfg(test)]
#[path = "composer_file_tiles_tests.rs"]
pub(crate) mod tests;
