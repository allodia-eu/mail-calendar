//! Files the user adds to a draft: the Attach button's picker, the list of what is attached, and
//! the drop target that takes a file dragged onto the composer.
//!
//! Split out of [`super::composer`] so that file stays the composer's chrome and the editor host.
//!
//! **A drop is handled natively, not by the page.** The editor bundle refuses `drop` (see
//! `main.ts`), because web code only ever sees a `File` with no path: it could neither hand the
//! bytes to Rust for a streamed send nor put a removable row in the list below. The host gets the
//! path, so both work, and the page is handed a picture only when the user asks for one.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, gdk, gio, glib};
use serde_json::json;
use webkit6::prelude::WebViewExt;

use super::composer_model::PickedFile;
use crate::l10n;

/// The responses of the drop question. Ids rather than labels, so the answer does not move when a
/// translation does.
const RESPONSE_INLINE: &str = "inline";
const RESPONSE_ATTACH: &str = "attach";
const RESPONSE_CANCEL: &str = "cancel";

/// One selected or dropped file as the composer records it: the path Rust reads the bytes from on
/// send, plus the name and media type the outgoing MIME part carries.
pub(super) fn picked_file(path: &Path) -> Option<PickedFile> {
    let file_name = path.file_name()?.to_str()?.to_owned();
    let (content_type, _) = gio::content_type_guess(Some(Path::new(&file_name)), None);
    let media_type = gio::content_type_get_mime_type(&content_type).map_or_else(
        || "application/octet-stream".to_owned(),
        |value| value.to_string(),
    );
    Some(PickedFile {
        path: path.to_string_lossy().into_owned(),
        file_name,
        media_type,
    })
}

pub(super) fn connect_file_picker(
    button: &gtk::Button,
    list: &gtk::ListBox,
    files: &Rc<RefCell<Vec<PickedFile>>>,
    window: &adw::ApplicationWindow,
) {
    let list = list.clone();
    let files = Rc::clone(files);
    let parent = window.clone();
    button.connect_clicked(move |_| {
        let dialog = gtk::FileDialog::new();
        let list = list.clone();
        let files = Rc::clone(&files);
        let parent = parent.clone();
        glib::MainContext::default().spawn_local(async move {
            let Ok(selected) = dialog.open_multiple_future(Some(&parent)).await else {
                return;
            };
            for index in 0..selected.n_items() {
                if let Some(path) = selected
                    .item(index)
                    .and_downcast::<gio::File>()
                    .and_then(|file| file.path())
                    && let Some(picked) = picked_file(&path)
                {
                    files.borrow_mut().push(picked);
                }
            }
            render_files(&list, &files);
        });
    });
}

/// How a picture reaches the editor.
///
/// A closure rather than the web view itself, so the drop target can be built in a test without
/// constructing a `WebView`: starting WebKit inside a test process aborts it on exit.
pub(super) type ShowPicture = Rc<dyn Fn(&Path, &str)>;

/// What a drop turns into, before anything is shown or attached.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct DroppedFiles {
    /// Files with nothing to decide: attached straight away.
    pub(super) attach: Vec<PathBuf>,
    /// Pictures, which the user is asked about.
    pub(super) pictures: Vec<PathBuf>,
}

/// Sorts a drop into the part that needs a question and the part that does not.
///
/// A picture raises the question the other formats do not: it can be shown where the user is
/// typing, or sent as a file to download. Everything else is simply attached, because there is
/// nothing else it could sensibly be.
///
/// The desktop's guess from the file name is enough to choose the question; the core sniffs the
/// bytes before anything is shown ([`mailcal_bindings::composer_image_data_url`]), so a
/// mislabelled file still cannot become an `<img>`.
pub(super) fn sort_drop(paths: Vec<PathBuf>) -> DroppedFiles {
    let (pictures, attach) = paths
        .into_iter()
        .filter(|path| picked_file(path).is_some())
        .partition(|path| {
            picked_file(path).is_some_and(|file| file.media_type.starts_with("image/"))
        });
    DroppedFiles { attach, pictures }
}

/// Accepts files dragged onto the composer. The question a picture raises is asked once for the
/// whole drop, not once per file.
pub(super) fn install_drop_target(
    target: &impl IsA<gtk::Widget>,
    show: ShowPicture,
    list: &gtk::ListBox,
    files: &Rc<RefCell<Vec<PickedFile>>>,
    window: &adw::ApplicationWindow,
    error: &gtk::Label,
) {
    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    // Capture, not bubble: the WebView installs a drop target of its own, and a bubble-phase
    // controller on an ancestor would never run for a drop over the editor, which is most of the
    // composer's area.
    drop.set_propagation_phase(gtk::PropagationPhase::Capture);
    let list = list.clone();
    let files = Rc::clone(files);
    let parent = window.clone();
    let error = error.clone();
    drop.connect_drop(move |_, value, _, _| {
        let Ok(dropped) = value.get::<gdk::FileList>() else {
            return false;
        };
        let sorted = sort_drop(dropped.files().iter().filter_map(gio::File::path).collect());
        if sorted.attach.is_empty() && sorted.pictures.is_empty() {
            return false;
        }
        attach_all(&sorted.attach, &list, &files);
        if !sorted.pictures.is_empty() {
            ask_and_place(
                sorted.pictures,
                Rc::clone(&show),
                list.clone(),
                Rc::clone(&files),
                parent.clone(),
                error.clone(),
            );
        }
        true
    });
    target.as_ref().add_controller(drop);
}

fn attach_all(paths: &[PathBuf], list: &gtk::ListBox, files: &Rc<RefCell<Vec<PickedFile>>>) {
    if paths.is_empty() {
        return;
    }
    for path in paths {
        if let Some(picked) = picked_file(path) {
            files.borrow_mut().push(picked);
        }
    }
    render_files(list, files);
}

/// Asks how the dropped pictures should go into the message, then does it.
fn ask_and_place(
    pictures: Vec<PathBuf>,
    show: ShowPicture,
    list: gtk::ListBox,
    files: Rc<RefCell<Vec<PickedFile>>>,
    parent: adw::ApplicationWindow,
    error: gtk::Label,
) {
    let dialog = adw::AlertDialog::new(
        Some(l10n::compose_image_drop_title()),
        Some(l10n::compose_image_drop_body()),
    );
    dialog.add_response(RESPONSE_CANCEL, l10n::action_cancel());
    dialog.add_response(RESPONSE_ATTACH, l10n::compose_image_drop_attach());
    dialog.add_response(RESPONSE_INLINE, l10n::compose_image_drop_inline());
    dialog.set_response_appearance(RESPONSE_INLINE, adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some(RESPONSE_INLINE));
    dialog.set_close_response(RESPONSE_CANCEL);
    glib::MainContext::default().spawn_local(async move {
        match dialog.choose_future(Some(&parent)).await.as_str() {
            RESPONSE_ATTACH => attach_all(&pictures, &list, &files),
            RESPONSE_INLINE => {
                // A picture the core cannot read as one is attached rather than dropped on the
                // floor: the user asked for it to be in the message, and losing it silently would
                // be the worse answer.
                let mut unreadable = Vec::new();
                for path in pictures {
                    let read = mailcal_bindings::composer_image_data_url(
                        path.to_string_lossy().into_owned(),
                    );
                    match read {
                        Ok(data_url) => show(&path, &data_url),
                        Err(_) => unreadable.push(path),
                    }
                }
                if !unreadable.is_empty() {
                    error.set_text(l10n::compose_image_failed());
                    error.set_visible(true);
                    attach_all(&unreadable, &list, &files);
                }
            }
            _ => {}
        }
    });
}

/// Hands one picture to the shared editor, which inserts it at the caret and records the inline
/// attachment the core turns into a `cid:` part on send.
pub(super) fn show_in_message(editor: &webkit6::WebView) -> ShowPicture {
    let editor = editor.clone();
    Rc::new(move |path: &Path, data_url: &str| {
        let payload = json!({
            "data_url": data_url,
            "file_name": path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
        });
        editor.evaluate_javascript(
            &format!("window.insertComposerImage({payload});"),
            None,
            None,
            None::<&gio::Cancellable>,
            |_| {},
        );
    })
}

pub(super) fn render_files(list: &gtk::ListBox, files: &Rc<RefCell<Vec<PickedFile>>>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for (index, file) in files.borrow().iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(&file.file_name)
            .subtitle(&file.media_type)
            .use_markup(false)
            .build();
        let remove = gtk::Button::from_icon_name("user-trash-symbolic");
        remove.set_tooltip_text(Some(l10n::action_remove()));
        remove.update_property(&[AccessibleProperty::Label(l10n::action_remove())]);
        let list_for_remove = list.clone();
        let files = Rc::clone(files);
        remove.connect_clicked(move |_| {
            if index < files.borrow().len() {
                files.borrow_mut().remove(index);
                render_files(&list_for_remove, &files);
            }
        });
        row.add_suffix(&remove);
        list.append(&row);
    }
}

#[cfg(test)]
#[path = "composer_attach_tests.rs"]
pub(crate) mod tests;
