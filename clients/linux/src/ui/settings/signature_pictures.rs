//! Pictures into the signature editor.
//!
//! A picture reaches it three ways, and they end in one place ([`place`]): the Add image button,
//! and, because this toolkit's WebView tells the page nothing about the clipboard
//! ([`crate::ui::editor_paste`]), a paste of pixels or of a copied file. Each asks the core's rule
//! for a signature's pictures ([`signature_image`]) before the picture goes in, and a picked or
//! copied file asks it before it is read.

use std::rc::Rc;

use gtk::{gio, glib, prelude::*};
use serde_json::json;
use webkit6::prelude::WebViewExt;

use crate::{
    l10n,
    ui::{
        editor_paste::{PasteAnswer, PastedPicture},
        signature_image::{self, SignatureImage},
    },
};

/// The types the file picker offers: the three the core takes in a signature
/// (`docs/signatures.md`).
///
/// Each type is named, never `add_mime_type("image/*")`: a `GtkFileFilter` matches a file's
/// content type against the types it was given, and `image/*` is not one, so a wildcard filter
/// matches **nothing** and the picker shows an empty directory.
const PICKED_TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/gif"];

/// Opens the file picker from `button`, and puts the picture picked into `editor`.
pub(super) fn connect_image_picker(
    button: &gtk::Button,
    editor: &webkit6::WebView,
    error: &gtk::Label,
    window: &gtk::Window,
) {
    let editor = editor.clone();
    let error = error.clone();
    let parent = window.clone();
    button.connect_clicked(move |_| {
        let dialog = gtk::FileDialog::new();
        let (filters, images) = image_filters();
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&images));
        let editor = editor.clone();
        let error = error.clone();
        let parent = parent.clone();
        glib::MainContext::default().spawn_local(async move {
            let Ok(file) = dialog.open_future(Some(&parent)).await else {
                return;
            };
            error.set_visible(false);
            place(&editor, &error, read_image(&file).await);
        });
    });
}

/// Reads a picked file into a `data:` URI.
///
/// The core's rule is asked with the type and size from the file's own metadata **before**
/// anything is read: the person may pick a four-gigabyte file from a mounted cloud share, and
/// pulling all of it into memory in order to then refuse it is not a thing to do on the main loop.
async fn read_image(file: &gio::File) -> SignatureImage {
    let Ok(info) = file
        .query_info_future(
            "standard::size,standard::content-type",
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::DEFAULT,
        )
        .await
    else {
        return SignatureImage::Failed;
    };
    let media_type = info.content_type();
    // A negative size means "unknown", not "empty": the bytes are measured again once read.
    let size = u64::try_from(info.size()).unwrap_or(0);
    if let Some(refused) = signature_image::refusal(media_type.as_deref(), size) {
        return SignatureImage::Refused(refused);
    }
    let Ok((bytes, _)) = file.load_contents_future().await else {
        return SignatureImage::Failed;
    };
    let alt_text = file
        .basename()
        .and_then(|name| {
            std::path::Path::new(&name)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    signature_image::signature_image(&bytes, media_type.as_deref(), &alt_text)
}

fn image_filters() -> (gio::ListStore, gtk::FileFilter) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(l10n::settings_signatures_insert_image()));
    for media_type in PICKED_TYPES {
        filter.add_mime_type(media_type);
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    (filters, filter)
}

/// Puts one picture into the signature at the caret, or says why it could not go in, in the same
/// words however the person offered it.
fn place(editor: &webkit6::WebView, error: &gtk::Label, image: SignatureImage) {
    match image {
        SignatureImage::DataUrl { value, alt_text } => {
            let payload = json!({ "data_url": value, "alt_text": alt_text }).to_string();
            let argument = json!(payload).to_string();
            editor.evaluate_javascript(
                &format!("window.insertSignatureImage({argument});"),
                None,
                None,
                None::<&gio::Cancellable>,
                |_| {},
            );
        }
        SignatureImage::Refused(refused) => {
            show(error, &signature_image::image_refusal_text(refused));
        }
        SignatureImage::Failed => show(error, l10n::settings_signatures_image_failed()),
    }
}

/// What a paste into the signature editor does.
///
/// Pixels are judged by the core's rule with no name to give them; a copied **file** is read
/// exactly as the Add image button reads a picked one. There is no attachment list here to fall
/// back on, so a file the signature cannot take is refused with the line the button uses.
pub(super) fn paste_answer(editor: &webkit6::WebView, error: &gtk::Label) -> PasteAnswer {
    // Weak: the editor's own context menu holds this answer.
    let picture_editor = editor.downgrade();
    let picture_error = error.clone();
    let files_editor = editor.downgrade();
    let files_error = error.clone();
    PasteAnswer {
        picture: Rc::new(move |picture: Option<PastedPicture>| {
            let Some(editor) = picture_editor.upgrade() else {
                return;
            };
            picture_error.set_visible(false);
            let read = picture.map_or(SignatureImage::Failed, |picture| {
                signature_image::signature_image(&picture.bytes, Some(&picture.media_type), "")
            });
            place(&editor, &picture_error, read);
        }),
        files: Rc::new(move |paths| {
            if paths.is_empty() {
                return false;
            }
            files_error.set_visible(false);
            let editor = files_editor.clone();
            let error = files_error.clone();
            glib::MainContext::default().spawn_local(async move {
                for path in paths {
                    let image = read_image(&gio::File::for_path(&path)).await;
                    // The editor may have closed while the file was read.
                    let Some(editor) = editor.upgrade() else {
                        return;
                    };
                    place(&editor, &error, image);
                }
            });
            true
        }),
    }
}

fn show(label: &gtk::Label, message: &str) {
    label.set_text(message);
    label.set_visible(true);
}

#[cfg(test)]
mod tests {
    /// The picker offers the three types the core takes, and nothing else.
    #[test]
    fn the_picker_offers_png_jpeg_and_gif_only() {
        assert_eq!(
            super::PICKED_TYPES,
            ["image/png", "image/jpeg", "image/gif"]
        );
        for media_type in super::PICKED_TYPES {
            assert_eq!(
                super::signature_image::refusal(Some(media_type), 1),
                None,
                "{media_type} is offered, so the core takes it"
            );
        }
    }
}
