//! What the composer accepts from a drop, and where the drop target has to listen.
//!
//! The GTK half is called from the crate's single `gtk::init` test (see
//! [`super::super::mailbox::tests`]); the rest is plain unit tests.

use std::{cell::RefCell, path::Path, rc::Rc};

use adw::prelude::*;

use super::{
    PictureAnswer, accept_files, install_drop_target, picked_file, render_files, sort_drop,
};
use crate::ui::composer_model::PickedFile;

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

/// The drop target listens in the **capture** phase, and the attachment list follows what is on it.
///
/// The phase is the part that fails silently: the WebView installs a drop target of its own, so a
/// bubble-phase controller on an ancestor never runs for a drop over the editor, which is most of
/// the composer's area. The drop would simply do nothing, on a build that compiles.
pub(crate) fn the_drop_target_listens_ahead_of_the_web_view() {
    let window = adw::ApplicationWindow::builder().build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let list = gtk::ListBox::new();
    let error = gtk::Label::new(None);
    let files = Rc::new(RefCell::new(Vec::<PickedFile>::new()));

    let accept = accept_files(
        PictureAnswer::Ask,
        Rc::new(|_, _| {}),
        &list,
        &files,
        &window,
        &error,
    );
    install_drop_target(&content, accept);

    let controllers = content.observe_controllers();
    let drop = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|item| item.downcast::<gtk::DropTarget>().ok())
        .expect("the composer accepts dropped files");
    assert_eq!(drop.propagation_phase(), gtk::PropagationPhase::Capture);
    assert!(drop.actions().contains(gtk::gdk::DragAction::COPY));

    // What a drop leaves behind: one removable row per file, named as the recipient will see it.
    files.borrow_mut().push(PickedFile {
        path: "/tmp/report.pdf".to_owned(),
        file_name: "report.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
    });
    render_files(&list, &files);
    let row = list
        .first_child()
        .and_downcast::<adw::ActionRow>()
        .expect("an attached file gets a row");
    assert_eq!(row.title(), "report.pdf");
    assert!(!row.uses_markup(), "a file name is never parsed as markup");
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
    let list = gtk::ListBox::new();
    let error = gtk::Label::new(None);
    let files = Rc::new(RefCell::new(Vec::<PickedFile>::new()));
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
        &list,
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
    assert!(files.borrow().is_empty(), "and it is not attached as well");

    shown.borrow_mut().clear();
    let drop = accept_files(
        PictureAnswer::Ask,
        Rc::clone(&recorder) as super::ShowPicture,
        &list,
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
