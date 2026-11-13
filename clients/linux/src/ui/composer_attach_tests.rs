//! What the composer accepts from a drop, and where the drop target has to listen.
//!
//! The GTK half is called from the crate's single `gtk::init` test (see
//! [`super::super::mailbox::tests`]); the rest is plain unit tests.

use std::{cell::RefCell, path::Path, rc::Rc, sync::Arc};

use adw::prelude::*;

use super::{PictureAnswer, accept_files, install_drop_target, picked_file, sort_drop};
use crate::ui::composer_file_tiles::{ComposerFileStore, FileTiles, tests::FakeStore};

#[test]
fn a_dropped_file_carries_the_name_and_media_type_its_mime_part_needs() {
    let picked = picked_file(Path::new("/tmp/quarterly report.pdf")).expect("a named file");
    assert_eq!(picked.file_name, "quarterly report.pdf");
    assert_eq!(picked.media_type, "application/pdf");
    assert_eq!(picked.path, "/tmp/quarterly report.pdf");
    // A path with no file name is not something to attach.
    assert!(picked_file(Path::new("/")).is_none());
}

#[test]
fn only_the_pictures_in_a_drop_raise_the_question() {
    // Everything else has one sensible answer, so asking would be a dialog with nothing to
    // decide. A mixed drop attaches the rest straight away and asks about the pictures once.
    let sorted = sort_drop(
        ["/tmp/screenshot.png", "/tmp/report.pdf", "/tmp/holiday.JPG"]
            .into_iter()
            .map(Into::into)
            .collect(),
    );

    assert_eq!(
        sorted.pictures,
        vec![
            std::path::PathBuf::from("/tmp/screenshot.png"),
            std::path::PathBuf::from("/tmp/holiday.JPG"),
        ]
    );
    assert_eq!(
        sorted.attach,
        vec![std::path::PathBuf::from("/tmp/report.pdf")]
    );
    assert_eq!(sort_drop(Vec::new()), super::DroppedFiles::default());
}

fn tiles(store: &Arc<FakeStore>) -> Rc<FileTiles> {
    FileTiles::new(Some(Arc::clone(store) as Arc<dyn ComposerFileStore>), "c")
}

/// The drop target listens in the **capture** phase, and what a drop leaves behind is in the
/// core's list.
///
/// The phase is the part that fails silently: the WebView installs a drop target of its own, so a
/// bubble-phase controller on an ancestor never runs for a drop over the editor, which is most of
/// the composer's area. The drop would simply do nothing, on a build that compiles.
pub(crate) fn the_drop_target_listens_ahead_of_the_web_view() {
    let window = adw::ApplicationWindow::builder().build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let error = gtk::Label::new(None);
    let store = Arc::new(FakeStore::default());
    let files = tiles(&store);

    let accept = accept_files(
        PictureAnswer::Ask,
        Rc::new(|_, _| {}),
        &files,
        &window,
        &error,
    );
    install_drop_target(&content, Rc::clone(&accept));

    let controllers = content.observe_controllers();
    let drop = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|item| item.downcast::<gtk::DropTarget>().ok())
        .expect("the composer accepts dropped files");
    assert_eq!(drop.propagation_phase(), gtk::PropagationPhase::Capture);
    assert!(drop.actions().contains(gtk::gdk::DragAction::COPY));

    // What a drop leaves behind: the files the core holds, named as the recipient will see them,
    // and a tile for each.
    assert!(accept(vec![
        "/tmp/report.pdf".into(),
        "/tmp/notes.txt".into()
    ]));
    assert_eq!(store.names(), ["report.pdf", "notes.txt"]);
    assert_eq!(
        files.widget().observe_children().n_items(),
        2,
        "one tile per file"
    );
    assert!(!accept(Vec::new()), "nothing to take is not taken");
}

/// A pasted picture goes **straight into the message**; a dropped one is asked about.
///
/// The gestures differ in what they already say. A drop lands on the composer as a whole, so both
/// answers are live. A paste lands at the caret: the user put the caret in the message and asked
/// for the picture there, and a dialog at that point argues with what they just did. Everything
/// that is not a picture is attached either way.
///
/// Asserted through the `show` seam rather than through a dialog, because the absence of a dialog
/// is exactly what a test of the paste has to prove and a modal cannot be waited for here: a
/// `ShowInMessage` batch reaches `show` synchronously, and an `Ask` batch reaches nothing until
/// someone answers.
pub(crate) fn a_paste_shows_the_picture_and_a_drop_asks_about_it() {
    let window = adw::ApplicationWindow::builder().build();
    let error = gtk::Label::new(None);
    let store = Arc::new(FakeStore::default());
    let files = tiles(&store);
    let shown = Rc::new(RefCell::new(Vec::<String>::new()));

    let dir = std::env::temp_dir().join("mailcal-paste-inline-test");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let picture = dir.join("holiday.png");
    std::fs::write(&picture, b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR").expect("write");

    let recorder = {
        let shown = Rc::clone(&shown);
        Rc::new(move |path: &Path, _: &str| {
            shown.borrow_mut().push(path.to_string_lossy().into_owned());
        })
    };

    let paste = accept_files(
        PictureAnswer::ShowInMessage,
        Rc::clone(&recorder) as super::ShowPicture,
        &files,
        &window,
        &error,
    );
    assert!(paste(vec![picture.clone()]));
    assert_eq!(
        shown.borrow().as_slice(),
        [picture.to_string_lossy().into_owned()],
        "a pasted picture is in the message without anything being asked"
    );
    assert!(store.names().is_empty(), "and it is not attached as well");

    shown.borrow_mut().clear();
    let drop = accept_files(
        PictureAnswer::Ask,
        Rc::clone(&recorder) as super::ShowPicture,
        &files,
        &window,
        &error,
    );
    assert!(drop(vec![picture.clone()]));
    assert!(
        shown.borrow().is_empty(),
        "a dropped picture waits for the user to say what it is for"
    );

    std::fs::remove_dir_all(&dir).ok();
}
