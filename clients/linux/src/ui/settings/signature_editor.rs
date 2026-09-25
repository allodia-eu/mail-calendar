//! The signature body editor: the shared editor bundle hosted body-only inside Settings.
//!
//! It loads the same bundle through the same [`SecureWebView`] the composer uses, so the two hosts
//! carry one definition of the gates rather than two that can drift; authoring a signature *is*
//! authoring mail content. The one thing it does that the composer does not is insert an image as
//! a self-contained `data:` URI: that is what a signature stores (one file, no side-car blobs to
//! lose) and what the core rewrites to a `cid:` part on send.
//!
//! A picture reaches it three ways, and they end in one place ([`place`]): the Insert image button,
//! and, because this toolkit's WebView tells the page nothing about the clipboard
//! ([`crate::ui::editor_paste`]), a paste of pixels or of a copied file. A signature's rule is its
//! own, not the message body's: [`signature_image`] caps it far lower, because a signature rides in
//! every message the account sends.

use std::{rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::MailcalApp;
use serde_json::json;
use webkit6::prelude::WebViewExt;

use super::{PageContext, page_box};
use crate::{
    l10n,
    ui::{
        composer::{EDITOR_HTML, editor_labels},
        editor_paste::{self, PasteAnswer, PastedPicture},
        signature_image::{self, SignatureImage},
        webview::{DocumentKind, SecureWebView},
    },
};

/// What the editor is open for. A `None` id is a create: the editor is the same either way, only
/// its title and what Save dispatches differ.
pub(super) struct EditingSignature {
    pub(super) id: Option<String>,
    pub(super) name: String,
    pub(super) body_html: String,
}

/// Opens the editor as a Settings detail. `on_saved` runs after the core stores the signature.
pub(super) fn open(ctx: &PageContext, editing: EditingSignature, on_saved: impl Fn() + 'static) {
    let title = if editing.id.is_none() {
        l10n::settings_signatures_add().to_owned()
    } else {
        editing.name.clone()
    };
    if let Some(previous) = ctx.navigation.child_by_name("signature-editor") {
        ctx.navigation.remove(&previous);
    }
    let page = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(&title, "")));
    let navigation = ctx.navigation.downgrade();
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    cancel.connect_clicked(move |_| {
        let navigation = navigation.clone();
        glib::idle_add_local_once(move || leave_editor(&navigation));
    });
    header.pack_start(&cancel);
    let content = page_box(&title);

    let name = gtk::Entry::new();
    name.set_text(&editing.name);
    name.set_placeholder_text(Some(l10n::settings_signatures_name_placeholder()));
    // The row it sits in labels itself from its title and cannot lend that to the entry, so the
    // entry carries its own name; otherwise a screen reader reaches an unnamed text field.
    name.update_property(&[gtk::accessible::Property::Label(
        l10n::settings_signatures_name_label(),
    )]);
    let name_row = adw::ActionRow::builder()
        .title(l10n::settings_signatures_name_label())
        .use_markup(false)
        .build();
    name.set_valign(gtk::Align::Center);
    name.set_hexpand(true);
    name_row.add_suffix(&name);
    let names = adw::PreferencesGroup::new();
    names.add(&name_row);
    content.append(&names);

    let body_label = gtk::Label::new(Some(l10n::settings_signatures_body_label()));
    body_label.set_xalign(0.0);
    content.append(&body_label);

    let web = SecureWebView::new(DocumentKind::Composer, ctx.sender.clone());
    // WebKit does not put its document on the accessibility bus here, so the host frame carries
    // the name; the same reason the composer labels the box around its editor rather than the
    // view itself. Named once the bundle has parsed, in `seed_body`.
    let frame = gtk::Frame::new(None);
    frame.set_accessible_role(gtk::AccessibleRole::Group);
    frame.set_vexpand(true);
    frame.set_child(Some(web.widget()));
    content.append(&frame);
    seed_body(&web, &editing.body_html, &frame);

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let add_image = gtk::Button::with_label(l10n::settings_signatures_insert_image());
    connect_image_picker(&add_image, web.widget(), &error, &ctx.window);
    actions.append(&add_image);
    // A pasted picture takes the same road as a picked one. The controller goes on the frame around
    // the view, never on the view itself, for the reason `editor_paste::install` states.
    let paste = paste_answer(web.widget(), &error);
    editor_paste::install(&frame, web.widget(), paste.clone());
    web.set_paste_handler(paste);
    // A signature with no name is a row the user cannot tell apart in the picker, so Save waits
    // for one.
    let save = gtk::Button::with_label(l10n::settings_signatures_save());
    save.add_css_class("suggested-action");
    save.set_sensitive(!editing.name.trim().is_empty());
    let gated = save.clone();
    name.connect_changed(move |entry| gated.set_sensitive(!entry.text().trim().is_empty()));
    connect_save(
        &save,
        web.widget(),
        ctx.app.clone(),
        editing.id,
        name,
        ctx.navigation.downgrade(),
        on_saved,
    );
    header.pack_end(&save);
    content.append(&actions);
    content.append(&error);

    page.add_top_bar(&header);
    page.set_content(Some(&content));
    ctx.navigation.add_named(&page, Some("signature-editor"));
    ctx.navigation.set_visible_child_name("signature-editor");
    web.load(EDITOR_HTML, false);
}

fn leave_editor(navigation: &glib::WeakRef<gtk::Stack>) {
    let Some(navigation) = navigation.upgrade() else {
        return;
    };
    navigation.set_visible_child_name("settings");
    if let Some(editor) = navigation.child_by_name("signature-editor") {
        navigation.remove(&editor);
    }
}

/// Loads the signature into the bundle once it has parsed.
///
/// The labels go first and the body second: `setSignatureBody` carries **this** surface's
/// placeholder, and the bundle's default ("Write your message") is the composer's wording, which
/// is a lie here. A new signature is seeded too, empty body and all, because that call is what
/// carries the placeholder.
fn seed_body(web: &SecureWebView, body_html: &str, host: &gtk::Frame) {
    let labels = editor_labels();
    let body = json!(body_html).to_string();
    let placeholder = json!(l10n::settings_signatures_placeholder()).to_string();
    let host = host.clone();
    web.connect_finished(move |view| {
        host.update_property(&[gtk::accessible::Property::Label(
            l10n::settings_signatures_placeholder(),
        )]);
        // Writing the signature is the only thing this screen is for, so the caret opens in it.
        // Asked for rather than assumed: the shared bundle focuses nothing of its own accord,
        // because in the composer the caret belongs in To (docs/contacts.md §4).
        let script = format!(
            "window.setComposerLabels({labels});window.setSignatureBody({body}, {placeholder});\
             window.focusComposerBody();"
        );
        view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |_| {});
    });
}

fn connect_save(
    button: &gtk::Button,
    editor: &webkit6::WebView,
    app: Arc<MailcalApp>,
    id: Option<String>,
    name: gtk::Entry,
    navigation: glib::WeakRef<gtk::Stack>,
    on_saved: impl Fn() + 'static,
) {
    let editor = editor.clone();
    let on_saved = Rc::new(on_saved);
    button.connect_clicked(move |_| {
        let app = Arc::clone(&app);
        let id = id.clone();
        let name = name.text().trim().to_owned();
        let navigation = navigation.clone();
        let on_saved = Rc::clone(&on_saved);
        editor.evaluate_javascript(
            "window.signatureBody()",
            None,
            None,
            None::<&gio::Cancellable>,
            move |result| {
                let Some(draft) = result.ok().and_then(|value| parse_draft(&value.to_str())) else {
                    // The bundle has not parsed yet. Leave the editor open rather than storing an
                    // empty body over a signature the user was editing.
                    log::warn!("the signature editor is not ready to be saved yet");
                    return;
                };
                if let Some(id) = id {
                    app.update_signature(id, name, draft.0, draft.1);
                } else {
                    app.create_signature(name, draft.0, draft.1);
                }
                // On the next turn of the loop, not here: removing this Settings detail destroys
                // the WebView, and this callback belongs to that view. One turn later every
                // caller has returned and teardown is safe.
                glib::idle_add_local_once(move || {
                    on_saved();
                    leave_editor(&navigation);
                });
            },
        );
    });
}

/// The HTML to store and its plain-text rendering, as `window.signatureBody()` hands them back.
fn parse_draft(json: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let html = value.get("body_html")?.as_str()?.to_owned();
    let plain = value
        .get("body_plain")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some((html, plain))
}

fn connect_image_picker(
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
/// The size is taken from the file's own metadata **before** anything is read: the user may pick a
/// four-gigabyte file from a mounted cloud share, and pulling all of it into memory in order to
/// then refuse it is not a thing to do on the main loop.
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
    // A negative size means "unknown", not "empty"; fall through and let the byte-length check
    // below be the gate, as it is anyway.
    if u64::try_from(info.size()).unwrap_or(0) > signature_image::LIMIT_BYTES {
        return SignatureImage::TooLarge;
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
    signature_image::signature_image(&bytes, info.content_type().as_deref(), &alt_text)
}

/// The formats a signature image may be picked in: the ones a recipient's mail client draws, and
/// the same list the Windows picker offers.
///
/// Each type is named, never `add_mime_type("image/*")`: a `GtkFileFilter` matches a file's
/// content type against the types it was given, and `image/*` is not one, so a wildcard filter
/// matches **nothing** and the picker shows an empty directory. That reads as "there are no images
/// here", which is the worst way for a filter to be wrong.
const IMAGE_TYPES: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/bmp",
];

fn image_filters() -> (gio::ListStore, gtk::FileFilter) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(l10n::settings_signatures_insert_image()));
    for media_type in IMAGE_TYPES {
        filter.add_mime_type(media_type);
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    (filters, filter)
}

/// Puts one read image into the signature at the caret, or says why it could not go in.
///
/// The three routes in (the button, a pasted picture, a pasted file) all end here, so the size
/// message and the refusal read the same however the user offered the picture.
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
        SignatureImage::TooLarge => show(
            error,
            &l10n::settings_signatures_image_too_large(&signature_image::format_limit()),
        ),
        SignatureImage::Failed => show(error, l10n::settings_signatures_image_failed()),
    }
}

/// What a paste into the signature editor does.
///
/// Pixels are judged by the signature's own rule with no name to give them; a copied **file** is
/// read exactly as the Insert image button reads a picked one, so a picture offered either way is
/// measured and named the same. There is no attachment list here to fall back on, so a file that is
/// not a picture the signature can take is refused with the line the button uses.
fn paste_answer(editor: &webkit6::WebView, error: &gtk::Label) -> PasteAnswer {
    let picture_editor = editor.clone();
    let picture_error = error.clone();
    let files_editor = editor.clone();
    let files_error = error.clone();
    PasteAnswer {
        picture: Rc::new(move |picture: Option<PastedPicture>| {
            picture_error.set_visible(false);
            let read = picture.map_or(SignatureImage::Failed, |picture| {
                signature_image::signature_image(&picture.bytes, Some(&picture.media_type), "")
            });
            place(&picture_editor, &picture_error, read);
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
    use super::parse_draft;

    #[test]
    fn a_draft_is_read_back_by_the_keys_the_core_stores() {
        assert_eq!(
            parse_draft(r#"{"body_html":"<p>Ada</p>","body_plain":"Ada"}"#),
            Some(("<p>Ada</p>".to_owned(), "Ada".to_owned()))
        );
        // An images-only signature has no text at all; that is a signature, not a failure.
        assert_eq!(
            parse_draft(r#"{"body_html":"<img src=\"data:image/png;base64,AQ==\">"}"#),
            Some((
                "<img src=\"data:image/png;base64,AQ==\">".to_owned(),
                String::new()
            ))
        );
    }

    #[test]
    fn an_unparsed_bundle_never_stores_an_empty_body_over_a_signature() {
        // `evaluate_javascript` on a page whose hooks do not exist yet answers with something that
        // is not the draft. Saving then has to do nothing; storing what it read would replace the
        // signature the user opened with an empty one.
        assert_eq!(parse_draft("undefined"), None);
        assert_eq!(parse_draft(""), None);
        assert_eq!(parse_draft("{}"), None);
        assert_eq!(parse_draft(r#"{"body_plain":"Ada"}"#), None);
    }
}
