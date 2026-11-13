//! The tiles drawn from the core's list. The GTK half is called from the crate's single
//! `gtk::init` test (see [`super::super::mailbox::tests`]).

use std::{
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard},
};

use adw::prelude::*;
use mailcal_bindings::{
    ComposerFileAttachment, ComposerFileTile, ComposerFiles, ControlState, MailcalError,
};

use super::{ComposerFileStore, FileTiles};
use crate::ui::{controls::tests::spins, icons, setup_widget_tests::descendants};

/// A stand-in for the core's list, answering as a core does: each file is listed under the name
/// it was handed with an id of its own, and `changed` is whether the list differs from the one it
/// was opened with.
#[derive(Default)]
pub(crate) struct FakeStore(Mutex<Held>);

#[derive(Default)]
struct Held {
    files: Vec<ComposerFileTile>,
    opened_with: Vec<String>,
    next: u64,
    /// The Remove control each tile is drawn with.
    remove: Option<ControlState>,
}

impl Held {
    fn ids(&self) -> Vec<String> {
        self.files.iter().map(|tile| tile.id.clone()).collect()
    }

    fn answer(&self) -> ComposerFiles {
        ComposerFiles {
            tiles: self.files.clone(),
            changed: self.ids() != self.opened_with,
        }
    }
}

impl FakeStore {
    /// A composition that opened holding `names`, as a forward or a stored draft does.
    pub(crate) fn opened_with(names: &[&str]) -> Arc<Self> {
        let store = Arc::new(Self::default());
        let _ = store.add(String::new(), names.iter().map(|name| file(name)).collect());
        let mut held = store.held();
        held.opened_with = held.ids();
        drop(held);
        store
    }

    /// One whose tiles are drawn with `remove` as their Remove's state.
    fn removing(remove: ControlState) -> Arc<Self> {
        let store = Arc::new(Self::default());
        store.held().remove = Some(remove);
        store
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.held()
            .files
            .iter()
            .map(|tile| tile.file_name.clone())
            .collect()
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        self.0.lock().expect("fake store")
    }
}

impl ComposerFileStore for FakeStore {
    fn files(&self, _composition: String) -> Result<ComposerFiles, MailcalError> {
        Ok(self.held().answer())
    }

    fn add(
        &self,
        _composition: String,
        files: Vec<ComposerFileAttachment>,
    ) -> Result<ComposerFiles, MailcalError> {
        let mut held = self.held();
        let remove = held.remove.unwrap_or(ControlState {
            enabled: true,
            running: false,
        });
        for file in files {
            held.next += 1;
            let id = format!("file-{}", held.next);
            let size = std::fs::metadata(&file.path).ok().map(|meta| meta.len());
            held.files.push(ComposerFileTile {
                id,
                file_name: file.file_name,
                size,
                remove,
            });
        }
        Ok(held.answer())
    }

    fn remove(&self, _composition: String, file: String) -> Result<ComposerFiles, MailcalError> {
        let mut held = self.held();
        held.files.retain(|tile| tile.id != file);
        Ok(held.answer())
    }
}

pub(crate) fn file(name: &str) -> ComposerFileAttachment {
    ComposerFileAttachment {
        path: format!("/staged/{name}"),
        file_name: name.to_owned(),
        media_type: "application/octet-stream".to_owned(),
    }
}

fn tiles_on_screen(tiles: &FileTiles) -> Vec<gtk::Box> {
    descendants::<gtk::Box>(tiles.widget().upcast_ref())
        .into_iter()
        .filter(|tile| tile.widget_name() == "composer-file")
        .collect()
}

fn texts(tile: &gtk::Box) -> Vec<String> {
    descendants::<gtk::Label>(tile.upcast_ref())
        .iter()
        .map(|label| label.text().to_string())
        .collect()
}

/// Each tile is its file's name, shortened in the middle so the extension stays, with its size in
/// words beneath and a Remove that takes it off through the core. Whether the list changed is the
/// core's answer, so a forward's originals are not a change and taking one off is.
pub(crate) fn a_tile_names_its_file_and_its_remove_goes_through_the_core() {
    let dir = std::env::temp_dir().join("mailcal-file-tiles-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let report = dir.join("quarterly report for the board.pdf");
    std::fs::write(&report, vec![0_u8; 2048]).expect("write");

    let store = FakeStore::opened_with(&["original.csv"]);
    let tiles = FileTiles::new(Some(Arc::clone(&store) as Arc<dyn ComposerFileStore>), "c");
    assert_eq!(
        tiles_on_screen(&tiles).len(),
        1,
        "a forward opens holding its original"
    );
    assert!(!tiles.current().changed, "which is nothing the user did");
    assert!(tiles.widget().is_visible());

    tiles.add(vec![ComposerFileAttachment {
        path: report.to_string_lossy().into_owned(),
        file_name: "quarterly report for the board.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
    }]);
    let drawn = tiles_on_screen(&tiles);
    assert_eq!(drawn.len(), 2);
    assert!(tiles.current().changed);
    let added = &drawn[1];
    let name = descendants::<gtk::Label>(added.upcast_ref())
        .into_iter()
        .next()
        .expect("a tile names its file");
    assert_eq!(name.text(), "quarterly report for the board.pdf");
    assert_eq!(name.ellipsize(), gtk::pango::EllipsizeMode::Middle);
    assert!(!name.uses_markup(), "a file name is never parsed as markup");
    assert_eq!(
        added.tooltip_text().as_deref(),
        Some("quarterly report for the board.pdf"),
        "the whole name, where the tile had to shorten it"
    );
    assert_eq!(
        texts(added)[1],
        gtk::glib::format_size(2048).to_string(),
        "the size in the user's own units"
    );
    // A file that could not be read has no size to give.
    assert_eq!(texts(&drawn[0]).len(), 1);

    let remove = descendants::<gtk::Button>(drawn[0].upcast_ref())
        .into_iter()
        .next()
        .expect("each tile has a Remove");
    assert_eq!(remove.icon_name().as_deref(), Some(icons::REMOVE));
    assert!(remove.is_sensitive());
    remove.emit_clicked();
    assert_eq!(store.names(), ["quarterly report for the board.pdf"]);
    assert_eq!(
        tiles_on_screen(&tiles).len(),
        1,
        "redrawn from the core's answer"
    );
    assert!(
        tiles.current().changed,
        "taking an original off is a change"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The Remove is drawn from the tile's `remove` state through the client's control components,
/// and an empty list takes no room in the column.
pub(crate) fn a_tile_draws_its_remove_from_the_cores_state() {
    let store = FakeStore::removing(ControlState {
        enabled: false,
        running: true,
    });
    let tiles = FileTiles::new(Some(Arc::clone(&store) as Arc<dyn ComposerFileStore>), "c");
    assert!(!tiles.widget().is_visible(), "no files, no row");
    tiles.add(vec![file("plan.pdf")]);
    assert!(tiles.widget().is_visible());
    let remove = descendants::<gtk::Button>(tiles.widget().upcast_ref())
        .into_iter()
        .next()
        .expect("the tile has a Remove");
    assert!(!remove.is_sensitive() && spins(&remove));

    let coreless: Rc<FileTiles> = FileTiles::new(None, "c");
    coreless.add(vec![file("plan.pdf")]);
    assert!(
        tiles_on_screen(&coreless).is_empty(),
        "a composer with no core holds no files"
    );
}
